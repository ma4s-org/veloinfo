import htmx from "htmx.org";

class RouteSearching extends HTMLElement {
    constructor() {
        super();
        this.innerHTML = /*html*/`
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
                        <md-filled-button hx-on:click="document.querySelector('veloinfo-map').clear()" hx-target="#info">
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

        var end = window.start_marker.getLngLat();
        // get the position of the device
        document.getElementById("search_position_dialog").removeAttribute("style");
        document.getElementById("search_position_dialog").setAttribute("open", "true");
        var start = await new Promise((resolve, reject) => {
            navigator.geolocation.getCurrentPosition((position) => {
                resolve(position);
                document.getElementById("search_position_dialog").removeAttribute("open");
            });
        });
        if (this.map.getSource("searched_route") == null) {
            this.map.addSource("searched_route", {
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
            this.map.addLayer({
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
        let calculateBearing = document.querySelector('veloinfo-map').calculateBearing;
        var bearing = calculateBearing(
            start.coords.longitude,
            start.coords.latitude,
            end.lng,
            end.lat
        );
        let fitBounds = document.querySelector('veloinfo-map').fitBounds;
        var bounds = fitBounds([[start.coords.longitude, start.coords.latitude], [end.lng, end.lat]]);
        this.map.fitBounds(bounds, { bearing, pitch: 0, padding: window.innerHeight * .12, duration: 900 });

        this.socket = new WebSocket("/route/" + start.coords.longitude + "/" + start.coords.latitude + "/" + end.lng + "/" + end.lat);
        let coordinates = [];
        this.socket.onmessage = async (event) => {
            if (event.data.startsWith("<route-panel")) {
                this.socket.close();
                this.map.removeLayer("searched_route");
                this.map.removeSource("searched_route");
                coordinates = [];
                document.getElementById("info").innerHTML = event.data;
                htmx.process(document.getElementById("info"));
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
                    this.map.getSource("searched_route").setData(data);
                }
            }
        }
    }
}

customElements.define('route-searching', RouteSearching);