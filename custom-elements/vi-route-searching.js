export default class RouteSearching extends HTMLElement {
    constructor(viMain) {
        super();
        this.viMain = viMain;
        let innerHTML = /*html*/`
            <div class="absolute w-full max-h-[50%] overflow-auto md:w-[500px] bg-white z-20 bottom-0 rounded-lg">
                <div class="p-4">
                    <h2 class="text-xl font-bold">
                        Recherche de route
                    </h2>
                    <p>
                        Veuillez patienter pendant que nous recherchons votre itinéraire...
                    </p>
                </div>
                <div class="flex justify-center">
                        <md-filled-button id="cancel_button">
                            annuler
                        </md-filled-button>
                </div>
                <md-dialog id="search_position_dialog" style="display: none;">
                    <div slot="content" class="flex flex-col justify-center">
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
            console.log(`${err.name}, ${err.message}`);
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

        var end = window.start_marker.getLngLat();
        var start;

        // Si end_marker existe, on utilise les marqueurs existants (mode changeStartMode)
        if (this.viMain.end_marker) {
            console.log("Mode changeStartMode - utilisant les marqueurs existants");
            end = this.viMain.end_marker.getLngLat();
            start = { coords: { longitude: window.start_marker.getLngLat().lng, latitude: window.start_marker.getLngLat().lat } };
        } else {
            // Sinon, on demande la position GPS du départ
            console.log("Mode normal - demandant la géolocalisation");
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
        var bearing = calculateBearing(
            start.coords.longitude,
            start.coords.latitude,
            end.lng,
            end.lat
        );
        let fitBounds = this.viMain.fitBounds;
        var bounds = fitBounds([[start.coords.longitude, start.coords.latitude], [end.lng, end.lat]]);
        this.viMain.map.fitBounds(bounds, { bearing, pitch: 0, padding: window.innerHeight * .12, duration: 900 });

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
