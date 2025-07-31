import htmx from "htmx.org";

class RouteSearching extends HTMLElement {
    constructor() {
        super();
        this.innerHTML = `
            <div class="absolute w-full max-h-[50%] overflow-auto md:w-[500px] bg-white z-20 bottom-0 rounded-lg">
                <div class="p-4">
                    <h2 class="text-xl font-bold">Recherche de route</h2>
                    <p>Veuillez patienter pendant que nous recherchons votre itinéraire...</p>
                </div>
                <div class="flex justify-center">
                        <md-filled-button hx-on:click="clear()" hx-target="#info">annuler</md-filled-button>
                </div>
            </div>
        `;

        this.init();
        htmx.process(this);
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
        document.getElementById("search_route_dialog").removeAttribute("style");
        document.getElementById("search_route_dialog").setAttribute("open", "true");
        if (map.getSource("searched_route") == null) {
            map.addSource("searched_route", {
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
            map.addLayer({
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
        var bearing = calculateBearing(
            start.coords.longitude,
            start.coords.latitude,
            end.lng,
            end.lat
        );
        var bounds = fitBounds([[start.coords.longitude, start.coords.latitude], [end.lng, end.lat]]);
        map.fitBounds(bounds, { bearing, pitch: 0, padding: 30, duration: 900 });

        const socket = new WebSocket("/route/" + start.coords.longitude + "/" + start.coords.latitude + "/" + end.lng + "/" + end.lat);
        let coordinates = [];
        socket.onmessage = async (event) => {
            if (event.data.startsWith("<route-panel")) {
                socket.close();
                map.removeLayer("searched_route");
                map.removeSource("searched_route");
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
                    map.getSource("searched_route").setData(data);
                }
            }
        }
        document.getElementById("search_route_dialog").removeAttribute("open");

    }
}

customElements.define('route-searching', RouteSearching);