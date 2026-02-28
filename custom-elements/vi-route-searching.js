export default class RouteSearching extends HTMLElement {
    constructor(viMain) {
        super();
        this.viMain = viMain;
        let innerHTML = /*html*/`
            <div class="vi-panel">
                <div style="padding: 1rem;">
                    <h2 style="font-size: 1.25rem; font-weight: 700;">
                        Recherche de route
                    </h2>
                    <p>
                        Veuillez patienter pendant que nous recherchons votre itinéraire...
                    </p>
                </div>
                <div style="display: flex; flex-direction: row; justify-content: center; gap: 0.5em; padding-bottom: 1em; position: relative;">
                    <md-outlined-button id="change_start_button" style="position: absolute; left: 1em; transform: scale(0.75); transform-origin: left center; --md-sys-color-primary: #666666;">changer départ</md-outlined-button>
                    <md-filled-button id="cancel_button">annuler</md-filled-button>
                </div>
                <md-dialog id="search_position_dialog" style="display: none;">
                    <div slot="content" style="display: flex; flex-direction: column; justify-content: center;">
                        <div style="align-self: center;">
                            Recherche de votre position
                        </div>
                        <img src="/pub/search_location.png" style="width: 128px; align-self: center;" />
                    </div>
                </md-dialog>
            </div>
        `;
        this.innerHTML = innerHTML;

        this.init();
        htmx.process(this);
    }

    disconnectedCallback() {
        // Clean up any event listeners or resources
        if (this.socket) {
            this.socket.close();
        }
    }

    async init() {
        let wakeLock = null;
        // keep the screen open
        try {
            wakeLock = await navigator.wakeLock.request("screen");
        } catch (err) {
            // the wake lock request fails - usually system related, such being low on battery
        }
        document.addEventListener("visibilitychange", async () => {
            if (wakeLock !== null && document.visibilityState === "visible") {
                // Re-acquérir le verrou si nécessaire
                await navigator.wakeLock.request("screen");
            }
        });

        this.querySelector("#cancel_button").addEventListener("click", () => {
            if (this.socket) {
                this.socket.close();
            }
            this.viMain.clear();
        });

        var end = this.viMain.start_marker.getLngLat();
        var start;

        // Si end_marker existe, on utilise les marqueurs existants (mode changeStartMode)
        if (this.viMain.end_marker) {
            end = this.viMain.end_marker.getLngLat();
            start = { coords: { longitude: this.viMain.start_marker.getLngLat().lng, latitude: this.viMain.start_marker.getLngLat().lat } };
        } else {
            // Sinon, on demande la position GPS du départ
            // Changer la couleur du marqueur en bleu car c'est la destination
            if (this.viMain.start_marker) {
                this.viMain.start_marker.remove();
                this.viMain.start_marker = new maplibregl.Marker({ color: "#00f" }).setLngLat([end.lng, end.lat]).addTo(this.viMain.map);
            }
            // get the position of the device
            this.querySelector("#search_position_dialog").removeAttribute("style");
            this.querySelector("#search_position_dialog").setAttribute("open", "true");
            start = await new Promise((resolve, reject) => {
                navigator.geolocation.getCurrentPosition((position) => {
                    resolve(position);
                    this.querySelector("#search_position_dialog").removeAttribute("open");
                });
            });
        }
        this.querySelector("#change_start_button").addEventListener("click", () => {
            this.viMain.changeStartDestination = { lng: end.lng, lat: end.lat };
            if (this.viMain.map.getLayer("searched_route")) {
                this.viMain.map.removeLayer("searched_route");
            }
            if (this.viMain.map.getSource("searched_route")) {
                this.viMain.map.removeSource("searched_route");
            }
            this.viMain.querySelector("#info").innerHTML = "<vi-change-start></vi-change-start>";
        });

        if (this.viMain.map.getSource("searched_route") == null) {
            this.viMain.map.addSource("searched_route", {
                "type": "geojson",
                "data": {
                    "type": "Feature",
                    "properties": {},
                    "geometry": {
                        "type": "MultiLineString",
                        "coordinates": [start.coords.longitude, start.coords.latitude]
                    }
                }
            });
            this.viMain.map.addLayer({
                'id': 'searched_route',
                'source': 'searched_route',
                'type': 'line',
                "paint": {
                    "line-width": 8,
                    "line-color": "hsl(205, 100%, 50%)",
                    "line-blur": 0,
                    "line-opacity": 0.50
                }
            });
        }
        let calculateBearing = this.viMain.calculateBearing;
        let fitBounds = this.viMain.fitBounds;
        var bounds = fitBounds([[start.coords.longitude, start.coords.latitude], [end.lng, end.lat]]);
        this.viMain.map.fitBounds(bounds, { pitch: 0, padding: window.innerHeight * .12, duration: 900 });

        this.socket = new WebSocket("/route/" + start.coords.longitude + "/" + start.coords.latitude + "/" + end.lng + "/" + end.lat);
        let coordinates = [];
        this.socket.onmessage = async (event) => {
            if (event.data.startsWith("<vi-route-panel")) {
                this.socket.close();
                this.viMain.map.removeLayer("searched_route");
                this.viMain.map.removeSource("searched_route");
                coordinates = [];

                this.viMain.querySelector("#info").innerHTML = event.data;
                htmx.process(this.viMain.querySelector("#info"));

                // Mettre à jour l'URL après que le RoutePanel soit créé
                // Attendre un court instant pour que le custom element soit complètement initialisé
                setTimeout(() => {
                    const routePanel = this.viMain.querySelector("vi-route-panel");
                    if (routePanel) {
                        const coords = JSON.parse(routePanel.getAttribute('coordinates'));
                        if (coords && coords[0]) {
                            const safeCoords = coords[0];
                            this.viMain.updateRouteUrl(
                                safeCoords[0][0],
                                safeCoords[0][1],
                                safeCoords[safeCoords.length - 1][0],
                                safeCoords[safeCoords.length - 1][1]
                            );
                        }
                    }
                }, 100);

                return;
            } else {
                if (coordinates.length > 10000) {
                    coordinates = [];
                }
                coordinates.push(JSON.parse(event.data));
                if (coordinates.length % 1000 == 0) {
                    const data = {
                        "type": "Feature",
                        "properties": {},
                        "geometry": {
                            "type": "MultiLineString",
                            "coordinates": coordinates
                        }

                    };
                    this.viMain.map.getSource("searched_route").setData(data);
                }
            }
        }
        if (this.viMain.map.getLayer("selected")) {
            this.viMain.map.removeLayer("selected");
        }
    }
}

customElements.define('vi-route-searching', RouteSearching);
