class RoutePanel extends HTMLElement {
    constructor() {
        super();
        window.coordinates = JSON.parse(this.getAttribute('coordinates'));
        if (map.getLayer("selected")) {
            map.getSource("selected").setData({
                "type": "Feature",
                "properties": {},
                "geometry": {
                    "type": "MultiLineString",
                    "coordinates": [window.coordinates]
                }
            });
        } else {
            map.addSource("selected", {
                "type": "geojson",
                "data": {
                    "type": "Feature",
                    "properties": {},
                    "geometry": {
                        "type": "MultiLineString",
                        "coordinates": [window.coordinates]
                    }
                }
            })
            map.addLayer({
                "id": "selected",
                "type": "line",
                "source": "selected",
                "paint": {
                    "line-width": 8,
                    "line-color": "hsl(205, 100%, 50%)",
                    "line-blur": 0,
                    "line-opacity": 0.50
                }
            },
                "Road labels")
        }
        window.clearDistanceCache();
        let totalDistance = window.calculateTotalDistance(window.coordinates, 0).toFixed(1);
        let totalDuration = totalDistance / 15.0
        let durationString = "";
        let hours = Math.floor(totalDuration);
        let minutes = Math.round((totalDuration - hours) * 60.0)
        if (hours >= 1.0) {
            durationString = hours + " heures et "
        }
        durationString += ` ${minutes} minutes à 15 km/h`

        this.innerHTML = `
            <div class="absolute w-full max-h-[50%] overflow-auto md:w-[500px] bg-white z-20 bottom-0 rounded-lg">
                <div class="">
                    <div>
                        <div>
                            Longueur: <span style="font-weight: bold;">${totalDistance}</span> kms
                        </div>
                        <div>
                            <div>
                                Durée: ${durationString}
                            </div>
                        </div>
                        ${this.getAttribute('error')}
                    </div>
                    <div class="flex justify-center">
                        <md-filled-button hx-on:click="follow_route()" hx-target="#info">suivre</md-filled-button>
                        <md-filled-button hx-on:click="clear()" hx-target="#info">annuler</md-filled-button>
                    </div>
                </div>
            </div>
        `;

        var bearing = calculateBearing(
            window.coordinates[0][0],
            window.coordinates[0][1],
            window.coordinates[window.coordinates.length - 1][0],
            window.coordinates[window.coordinates.length - 1][1]);
        var bounds = fitBounds(window.coordinates);
        map.fitBounds(bounds, { bearing, pitch: 0, padding: 30, duration: 900 });
        (async () => {
            try {
                const wakeLock = await navigator.wakeLock.request("screen");
            } catch (err) {
                // the wake lock request fails - usually system related, such being low on battery
                console.log(`${err.name}, ${err.message}`);
            }
        })();

        window.follow_route = () => {
            htmx.ajax('GET', '/follow', {
                target: "#info"
            })
        };
    }
}

customElements.define('route-panel', RoutePanel);