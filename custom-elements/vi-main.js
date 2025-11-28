if ("serviceWorker" in navigator) {
    navigator.serviceWorker.register("/pub/service-worker.js");
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

class ViMain extends HTMLElement {
    constructor() {
        super();
        this.innerHTML = /*html*/`
            <div id="map">
                <a rel="me" href="https://mastodon.social/@MartinNHamel"></a>
                <vi-search-input id="search"></vi-search-input>
                <div id="info"></div>
                <vi-menu></vi-menu>
                <div id="buttons"
                    style="position: absolute; top:142px; right:6px; padding: 4px; z-index: 10">
                    <div id="layers_button"
                        style="border-radius: 0.375rem; border-width: 1px; border-color: rgb(209 213 219); cursor: pointer;">
                        <img style="width: 29px; height: 29px" class="bg-white rounded-md self-center" src="/pub/layers.png">
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
                    <div id="snow_button"
                        style="border-radius: 0.375rem; border-width: 1px; border-color: rgb(209 213 219); margin-top: 4px;
                               display: flex; justify-content: center; align-items: center; background-color: white;
                               width: 31px; height: 31px; cursor: pointer;">
                        <img style="width: 24px; height: 24px;" src="/pub/snow.png">
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
        this.addMap();

        this.querySelector('#layers_button').addEventListener('click', () => {
            const info = this.querySelector("#info");
            let viLayers = new ViLayers(this);
            info.innerHTML = ``;
            info.appendChild(viLayers);
        });

        this.querySelector('#snow_button').addEventListener('click', async () => {
            const map = this.map;
            const canvas = map.getCanvas();
            const centerPoint = map.project(map.getCenter());
            const dialog = this.querySelector('#city_snow_dialog');
            const features = map.queryRenderedFeatures(centerPoint, { layers: ['city'] });
            if (!features.length) return;
            const cityName = features[0].properties.name;
            dialog.querySelector('.city_name').textContent = cityName;
            dialog.show();

            const updateSnow = async (snow) => {
                await fetch('/city_snow_edit', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ name: cityName, snow })
                });
                dialog.close();
                map.getSource('city_snow').setUrl(`${window.location.origin}/city_snow?t=${Date.now()}`);
                map.getSource('bike_path').setUrl(`${window.location.origin}/bike_path?t=${Date.now()}`);
            };

            this.querySelector('#snow_yes').onclick = () => updateSnow(true);
            this.querySelector('#snow_no').onclick = () => updateSnow(false);
        });
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

        // Chargement des images
        (async () => {
            const images = [
                { name: 'bike-parking', url: '/pub/bicycle-parking.png' },
                { name: 'drinking-water', url: '/pub/drinking_water.png' },
                { name: 'bike-shop', url: '/pub/bike_shop.png' },
                { name: 'bicycle_repair_station', url: '/pub/bicycle_repair_station.png' },
                { name: 'bixi', url: '/pub/bixi.png' },
                { name: 'snow', url: '/pub/snow-margin.png' }
            ];
            for (const img of images) {
                const res = await this.map.loadImage(img.url);
                this.map.addImage(img.name, res.data);
            }
        })();

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

        this.map.on("load", () => {
            setTimeout(() => {
                const layers = JSON.parse(localStorage.getItem("layers"));
                ["bixi", "bike_parking", "bike_shop", "drinking_water", "bicycle_repair_station"].forEach(layer => {
                    const visible = !layers || !layers[layer] || layers[layer] !== "none";
                    this.map.setLayoutProperty(layer, 'visibility', visible ? 'visible' : 'none');
                });
            }, 1000);
            this.infoPanelUp();
        });

        this.map.on("click", (event) => {
            if (
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
                window.history.replaceState({}, "", `/?lat=${map.getCenter().lat}&lng=${map.getCenter().lng}&zoom=${map.getZoom()}`);
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
    }

    async infoPanelUp() {
        const bounds = this.map.getBounds();
        const r = await fetch(`/info_panel/up/${bounds._sw.lng}/${bounds._sw.lat}/${bounds._ne.lng}/${bounds._ne.lat}`);
        const json = await r.json();
        this.querySelector("#info").innerHTML = `<vi-info></vi-info>`;
        this.querySelector('vi-info').data = json;
    }

    async select(event) {
        if (window.start_marker && this.end_marker) {
            this.clear();
        }

        if (window.start_marker && this.map.getLayer("selected")) {
            this.selectBigger(event);
            return;
        }

        if (window.start_marker) window.start_marker.remove();
        window.start_marker = new maplibregl.Marker({ color: "#00f" }).setLngLat([event.lngLat.lng, event.lngLat.lat]).addTo(this.map);

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

    async selectBigger(event) {
        if (this.end_marker) this.end_marker.remove();
        this.end_marker = new maplibregl.Marker({ color: "#f00" }).setLngLat([event.lngLat.lng, event.lngLat.lat]).addTo(this.map);

        const r = await fetch(`/segment_panel_bigger/${window.start_marker.getLngLat().lng}/${window.start_marker.getLngLat().lat}/${event.lngLat.lng}/${event.lngLat.lat}`);
        const jsonData = await r.json();
        const segment_panel = new SegmentPanel(jsonData);
        this.querySelector("#info").innerHTML = ``;
        this.querySelector("#info").appendChild(segment_panel);
    }

    async clear() {
        if (window.start_marker) {
            window.start_marker.remove();
            window.start_marker = null;
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
}

customElements.define('vi-main', ViMain);

