if ("serviceWorker" in navigator) {
    navigator.serviceWorker.register("/pub/service-worker.js");
}

import '/custom-elements/FollowPanel.js';
import '/custom-elements/RoutePanel.js';
import RouteSearching from '/custom-elements/RouteSearching.js';
import '/custom-elements/SearchInput.js';
import '/custom-elements/VeloinfoMenu.js';
import '/custom-elements/VeloinfoInstallIos.js';
import '/custom-elements/VeloinfoInstallAndroid.js';
import '/custom-elements/vi-mobilizon-events.js';
import '/custom-elements/RouteDefine.js';
import '/custom-elements/SegmentPanel.js';
import '/custom-elements/PointPanel.js';
import '/custom-elements/ChangeStart.js';
import '/custom-elements/vi-info.js';



class ViMain extends HTMLElement {
    constructor() {
        super();
        let innerHTML = /*html*/`
            <div id="map">
                <a rel="me" href="https://mastodon.social/@MartinNHamel"></a>
                <search-input id="search"></search-input>
                <div id="info">
                </div>
                <veloinfo-menu></veloinfo-menu>
                <div id="buttons"
                    style="position: absolute; top:142px; right:6px; padding: 4px; z-index: 10">
                    <div
                        style="border-radius: 0.375rem; border-width: 1px; border-color: rgb(209 213 219); cursor: pointer;"
                        hx-get="/layers" hx-target="#info" hx-swap="innerHTML">
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
                               width: 31px; height: 31px; cursor: pointer;"
                    >
                        <img style="width: 24px; height: 24px;" src="/pub/snow.png">
                    </div>
                </div>
                <vi-mobilizon-events></vi-mobilizon-events>
            </div>
            <md-dialog id="city_snow_dialog" >
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
        this.innerHTML = innerHTML;
    }

    connectedCallback() {
        this.addMap();

        this.querySelector('#snow_button').addEventListener('click', () => {
            let map = this.map;

            // Obtenir le point central du canvas de la carte
            const centerPoint = map.getCenter();

            // Récupérer les éléments au centre de la carte sur city
            const dialog = this.querySelector('#city_snow_dialog');
            const features = map.queryRenderedFeatures(centerPoint, { layers: ['city'] });
            const cityName = features[0].properties.name;
            dialog.querySelector('.city_name').textContent = cityName;

            dialog.show();

            this.querySelector('#snow_yes').onclick = async () => {
                await fetch('/city_snow', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ name: cityName, snow: true })
                })
                dialog.close();

                document.querySelector('vi-main').insertCitySnow();
            };

            this.querySelector('#snow_no').onclick = async () => {
                await fetch('/city_snow', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ name: cityName, snow: false })
                });
                dialog.close();
                document.querySelector('vi-main').insertCitySnow();
            };

        });
    }

    addMap() {
        const position = JSON.parse(localStorage.getItem("position"));
        var lng = position?.lng || -73.39899762303611;
        var lat = position?.lat || 45.921066117828786;
        var zoom = position?.zoom || 6;
        let params = new URLSearchParams(window.location.search);
        if (params.has("lat") && params.has("lng") && params.has("zoom")) {
            lat = parseFloat(params.get("lat"));
            lng = parseFloat(params.get("lng"));
            zoom = parseFloat(params.get("zoom"));
        }

        // Speed
        var speed = 0;
        var speed_text = 0;

        navigator.geolocation.watchPosition((position) => {
            speed = position.coords.speed * 3.6;
            if (document.getElementById("speed_value")) {
                speed_text = document.getElementById("speed_value").textContent = speed?.toFixed(0) || 0;
                if (speed_text == 0 || speed == null) {
                    document.getElementById("speed_value").parentElement.style.display = "none";
                } else {
                    document.getElementById("speed_value").parentElement.style.display = "flex";
                }
            }
        });

        this.map = new maplibregl.Map({
            container: 'map',
            style: '/style.json',
            center: [lng, lat],
            zoom: zoom,
            minZoom: 8
        });


        // Load the images
        (async () => {
            const bike_image = await this.map.loadImage('/pub/bicycle-parking.png');
            this.map.addImage('bike-parking', bike_image.data);
            const drinking_water = await this.map.loadImage('/pub/drinking_water.png');
            this.map.addImage('drinking-water', drinking_water.data);
            const bike_shop = await this.map.loadImage('/pub/bike_shop.png');
            this.map.addImage('bike-shop', bike_shop.data);
            const bicycle_repair_station = await this.map.loadImage('/pub/bicycle_repair_station.png');
            this.map.addImage('bicycle_repair_station', bicycle_repair_station.data);
            const bixi = await this.map.loadImage('/pub/bixi.png');
            this.map.addImage('bixi', bixi.data);
            const snow = await this.map.loadImage('/pub/snow-margin.png');
            this.map.addImage('snow', snow.data);
        })();

        this.isGeolocateActive = false;
        this.map.addControl(new maplibregl.NavigationControl());
        this.geolocate = new maplibregl.GeolocateControl({
            fitBoundsOptions: {
                maxZoom: 16.5
            },
            positionOptions: {
                enableHighAccuracy: true
            },
            trackUserLocation: true
        });
        this.geolocate.on('trackuserlocationstart', () => { this.isGeolocateActive = true; });
        this.geolocate.on('trackuserlocationend', () => { this.isGeolocateActive = false; });
        this.geolocate.on('error', () => { this.isGeolocateActive = false; });
        this.map.addControl(this.geolocate);

        this.map.on("load", async () => {
            setTimeout(() => {
                const layers = JSON.parse(localStorage.getItem("layers"));
                ["bixi", "bike_parking", "bike_shop", "drinking_water", "bicycle_repair_station"].forEach(layer => {
                    if (!layers || !layers[layer]) {
                        this.map.setLayoutProperty(layer, 'visibility', 'visible');
                    } else if (layers[layer] == "none") {
                        this.map.setLayoutProperty(layer, 'visibility', 'none');
                    } else {
                        this.map.setLayoutProperty(layer, 'visibility', 'visible');
                    }
                });
            }, 1000);

            this.insertCitySnow();
            const bounds = this.map.getBounds();
            this.infoPanelUp();
        })

        let veloinfoMap = this;
        this.map.on("click", async function (event) {
            if (document.getElementById("info_panel_up") ||
                document.getElementById("info_panel_down") ||
                document.getElementById("segment_panel") ||
                document.getElementById("layers") ||
                document.getElementById("point_panel")
            ) {
                veloinfoMap.select(event);
            }
        });

        let timeout = null;
        let map = this.map;
        let that = this;
        this.map.on("move", () => {
            if (timeout) {
                clearTimeout(timeout);
            }
            timeout = setTimeout(() => {
                window.history.replaceState({}, "", "/?lat=" + map.getCenter().lat + "&lng=" + map.getCenter().lng + "&zoom=" + map.getZoom());
                const position = {
                    "lng": + map.getCenter().lng,
                    "lat": + map.getCenter().lat,
                    "zoom": + map.getZoom()
                }
                localStorage.setItem("position", JSON.stringify(position));
                if (that.querySelector('#info_panel_up')) {
                    that.infoPanelUp();
                }
            }, 1000);
        });
    }

    async infoPanelUp() {
        const bounds = this.map.getBounds();
        let r = await fetch("/info_panel/up/" + bounds._sw.lng + "/" + bounds._sw.lat + "/" + bounds._ne.lng + "/" + bounds._ne.lat);
        let json = await r.json();
        this.querySelector("#info").innerHTML = `<vi-info></vi-info>`
        this.querySelector('vi-info').data = json;
    }

    async insertCitySnow() {
        let r = await fetch("/city_snow_geojson");
        let geojson = await r.json();
        console.log(geojson);


        if (!geojson.features) {
            if (this.map.getLayer("city_snow")) {
                this.map.removeLayer("city_snow");
            }
            if (this.map.getSource("city_snow")) {
                this.map.removeSource("city_snow");
            }
            return;
        }

        if (this.map.getSource("city_snow")) {
            this.map.getSource("city_snow").setData(geojson);
        } else {
            this.map.addSource("city_snow", {
                "type": "geojson",
                "data": geojson
            });
            this.map.addLayer({
                "id": "city_snow",
                "type": "fill",
                "source": "city_snow",
                "layout": {
                    "visibility": "visible"
                },
                "paint": {
                    "fill-opacity": 0.5,
                    "fill-pattern": "snow"
                }
            }, "city");
        }
    }

    async select(event) {
        if (window.start_marker && this.end_marker) {
            this.clear();
        }

        if (window.start_marker && this.map.getLayer("selected")) {
            this.selectBigger(event);
            return;
        }

        if (window.start_marker) {
            window.start_marker.remove();
        }
        window.start_marker = new maplibregl.Marker({ color: "#00f" }).setLngLat([event.lngLat.lng, event.lngLat.lat]).addTo(this.map);

        let width = 20;
        var features = this.map.queryRenderedFeatures(
            [
                [event.point.x - width / 2, event.point.y - width / 2],
                [event.point.x + width / 2, event.point.y + width / 2]
            ], { layers: ['cycleway', 'designated', 'shared_lane'] });
        if (features.length) {
            var feature = features[0];
            let response = await fetch('/segment_panel_lng_lat/' + event.lngLat.lng + "/" + event.lngLat.lat);
            let jsonData = await response.json();
            this.querySelector("#info").innerHTML = `<segment-panel></segment-panel>`;
            this.querySelector("segment-panel").data = jsonData;
        } else {
            const selected = this.map.getSource("selected");

            if (selected) {
                selected.setData({
                    "type": "Feature",
                    "properties": {},
                    "geometry": {
                        "type": "LineString",
                        "coordinates": []
                    }
                });
            }

            //we find the nearest name
            let name = "";
            for (let i = 0; i < 1000; i = i + 10) {
                var features = this.map.queryRenderedFeatures(
                    [
                        [event.point.x - i, event.point.y - i],
                        [event.point.x + i, event.point.y + i]
                    ], { layers: ['name', 'Road network', 'City labels', 'Town labels', 'Village labels'] });
                features.forEach(f => {
                    if (f.properties.name) {
                        name = f.properties.name;
                        i = 1000;
                        return;
                    }
                });
                if (name) {
                    break;
                }
            }

            this.querySelector("#info").innerHTML = `<point-panel name="${name}"></point-panel>`;
            htmx.process(this.querySelector("#info"));
        }
    }

    async selectBigger(event) {
        if (this.end_marker) {
            this.end_marker.remove();
        }
        this.end_marker = new maplibregl.Marker({ color: "#f00" }).setLngLat([event.lngLat.lng, event.lngLat.lat]).addTo(this.map);

        let r = await fetch('/segment_panel_bigger/' + window.start_marker.getLngLat().lng + "/" + window.start_marker.getLngLat().lat + "/" + event.lngLat.lng + "/" + event.lngLat.lat);
        let jsonData = await r.json();
        this.querySelector("#info").innerHTML = `<segment-panel></segment-panel>`;
        this.querySelector("segment-panel").data = jsonData;
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
        if (this.map.getLayer("selected_safe")) {
            this.map.removeLayer("selected_safe");
        }
        if (this.map.getSource("selected_safe")) {
            this.map.removeSource("selected_safe");
        }
        if (this.map.getLayer("selected_fast")) {
            this.map.removeLayer("selected_fast");
        }
        if (this.map.getSource("selected_fast")) {
            this.map.removeSource("selected_fast");
        }
        if (this.map.getLayer("searched_route")) {
            this.map.removeLayer("searched_route");
        }
        if (this.map.getSource("searched_route")) {
            this.map.removeSource("searched_route");
        }
        if (this.map.getLayer("selected")) {
            this.map.removeLayer("selected");
        }
        if (this.map.getSource("selected")) {
            this.map.removeSource("selected");
        }

        // Display info panel
        let data = await (await fetch("/info_panel/down")).json();

        this.querySelector("#info").innerHTML = `<vi-info></vi-info>`;
        this.querySelector('vi-info').data = data;

    }

    async route() {
        let info = document.getElementById("info");
        let routeSearching = new RouteSearching(this.map);
        info.innerHTML = ``;
        info.appendChild(routeSearching);
    }

    fitBounds(geom) {
        var bounds = geom.reduce((currentBounds, coord) => {
            return [
                [Math.min(coord[0], currentBounds[0][0]), Math.min(coord[1], currentBounds[0][1])], // min coordinates
                [Math.max(coord[0], currentBounds[1][0]), Math.max(coord[1], currentBounds[1][1])]  // max coordinates
            ];
        }, [[Infinity, Infinity], [-Infinity, -Infinity]]);
        return bounds;
    }

    calculateBearing(lon1, lat1, lon2, lat2) {
        lon1 = lon1 * Math.PI / 180.0;
        lat1 = lat1 * Math.PI / 180.0;
        lon2 = lon2 * Math.PI / 180.0;
        lat2 = lat2 * Math.PI / 180.0;
        const y = Math.sin(lon2 - lon1) * Math.cos(lat2);
        const x = Math.cos(lat1) * Math.sin(lat2) - Math.sin(lat1) * Math.cos(lat2) * Math.cos(lon2 - lon1);
        let bearing = Math.atan2(y, x) * (180 / Math.PI);
        bearing = (bearing + 360) % 360; // Ensuring the bearing is positive
        return bearing;
    }

    calculateDistance(lat1, lon1, lat2, lon2) {
        const R = 6371; // Earth's radius in meters
        const φ1 = lat1 * Math.PI / 180;
        const φ2 = lat2 * Math.PI / 180;
        const Δφ = (lat2 - lat1) * Math.PI / 180;
        const Δλ = (lon2 - lon1) * Math.PI / 180;

        const a = Math.sin(Δφ / 2) * Math.sin(Δφ / 2) +
            Math.cos(φ1) * Math.cos(φ2) *
            Math.sin(Δλ / 2) * Math.sin(Δλ / 2);
        const c = 2 * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a));

        const distance = R * c;
        return distance;
    }

    calculateTotalDistance(coordinates, index = 0) {
        if (!coordinates || coordinates.length == 0) {
            return 0;
        }
        if (!this.distanceCache) {
            this.distanceCache = {};
        }
        let distanceCache = this.distanceCache;
        if (index in distanceCache) {
            return distanceCache[index];
        }

        let totalDistance = 0;
        for (let i = index; i < coordinates.length - 1; i++) {
            totalDistance += this.calculateDistance(
                coordinates[i][1], coordinates[i][0],
                coordinates[i + 1][1], coordinates[i + 1][0]
            );
        }

        distanceCache[index] = totalDistance;
        return totalDistance;
    }

    clearDistanceCache() {
        this.distanceCache = {};
    }
}

customElements.define('vi-main', ViMain);

