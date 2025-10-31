class RoutePanel extends HTMLElement {
    constructor() {
        super();
        const coordinates = JSON.parse(this.getAttribute('coordinates'));

        const safeCoordinates = coordinates[0];
        const fastCoordinates = coordinates[1];
        let map = document.querySelector('veloinfo-map').map;
        if (map.getLayer("selected_safe")) {
            map.getSource("selected_safe").setData({
                "type": "Feature",
                "properties": {},
                "geometry": {
                    "type": "MultiLineString",
                    "coordinates": [safeCoordinates]
                }
            });
        } else {
            map.addSource("selected_safe", {
                "type": "geojson",
                "data": {
                    "type": "Feature",
                    "properties": {},
                    "geometry": {
                        "type": "MultiLineString",
                        "coordinates": [safeCoordinates]
                    }
                }
            })
            map.addLayer({
                "id": "selected_safe",
                "type": "line",
                "source": "selected_safe",
                "paint": {
                    "line-width": 8,
                    "line-color": "hsl(205, 100%, 50%)",
                    "line-blur": 0,
                    "line-opacity": 0.50
                }
            },
                "Road labels")
        }

        if (map.getLayer("selected_fast")) {
            map.getSource("selected_fast").setData({
                "type": "Feature",
                "properties": {},
                "geometry": {
                    "type": "MultiLineString",
                    "coordinates": [fastCoordinates]
                }
            });
        } else {
            map.addSource("selected_fast", {
                "type": "geojson",
                "data": {
                    "type": "Feature",
                    "properties": {},
                    "geometry": {
                        "type": "MultiLineString",
                        "coordinates": [fastCoordinates]
                    }
                }
            })
            map.addLayer({
                "id": "selected_fast",
                "type": "line",
                "source": "selected_fast",
                "paint": {
                    "line-width": 8,
                    "line-color": "hsla(310, 100%, 50%, 1.00)",
                    "line-blur": 0,
                    "line-opacity": 0.50
                }
            },
                "Road labels")
        }
        console.log(safeCoordinates);
        console.log(fastCoordinates);



        document.querySelector('veloinfo-map').clearDistanceCache();
        let veloinfoMap = document.querySelector('veloinfo-map');
        let totalDistanceSafe = veloinfoMap.calculateTotalDistance(safeCoordinates).toFixed(1);
        veloinfoMap.clearDistanceCache();
        let totalDistanceFast = veloinfoMap.calculateTotalDistance(fastCoordinates).toFixed(1);
        let totalDurationSafe = totalDistanceSafe / 15.0
        let totalDurationFast = totalDistanceFast / 15.0
        let durationStringSafe = "";
        let hours = Math.floor(totalDurationSafe);
        let minutes = Math.round((totalDurationSafe - hours) * 60.0)
        if (hours >= 1.0) {
            durationStringSafe = hours + " heures et "
        }
        durationStringSafe += ` ${minutes} minutes à 15 km/h`
        let durationStringFast = "";
        hours = Math.floor(totalDurationFast);
        minutes = Math.round((totalDurationFast - hours) * 60.0)
        if (hours >= 1.0) {
            durationStringFast = hours + " heures et "
        }
        durationStringFast += ` ${minutes} minutes à 15 km/h`

        let innerHTML = /*html*/ ` 
            <div class="absolute w-full max-h-[50%] overflow-auto md:w-[500px] bg-white z-20 bottom-0 rounded-lg" 
                style="display: flex; justify-content: center; flex-direction: column">
                <div style="display: flex; flex-direction: row; justify-content: center; gap: 1em; padding: 1em;">
                    <md-filled-button hx-on:click="follow_route('safe')" hx-target="#info" style="--md-sys-color-primary: #c8dff5ff">
                        <div style="font-weight: bold;">
                            Itinéraire sécurisé
                        </div>
                        <div style="font-size: 0.9em;">
                            Longueur: <span style="font-weight: bold; font-size: 1.3em;">${totalDistanceSafe}</span> kms
                        </div>
                        ${this.getAttribute('error')}
                    </md-filled-button>
                    <md-filled-button hx-on:click="follow_route('fast')" hx-target="#info" style="--md-sys-color-primary: #ffcbfcff">
                        <div style="font-weight: bold;">
                            Itinéraire rapide
                        </div>
                        <div style="font-size: 0.9em;">
                            Longueur: <span style="font-weight: bold; font-size: 1.3em;">${totalDistanceFast}</span> kms
                        </div>
                        ${this.getAttribute('error')}
                    </md-filled-button>
                </div>
                <md-filled-button hx-on:click="document.querySelector('veloinfo-map').clear()" hx-target="#info">annuler</md-filled-button>
            </div>
        `;
        this.innerHTML = innerHTML;
        var bearing = document.querySelector('veloinfo-map').calculateBearing(
            safeCoordinates[0][0],
            safeCoordinates[0][1],
            safeCoordinates[safeCoordinates.length - 1][0],
            safeCoordinates[safeCoordinates.length - 1][1]);
        var bounds = document.querySelector('veloinfo-map').fitBounds(safeCoordinates);
        map.fitBounds(bounds, { bearing, pitch: 0, padding: window.innerHeight * .12, duration: 900 });


        window.follow_route = (route) => {
            let innerHTML = '<follow-panel route="' + route + '" coordinates="' + JSON.stringify(coordinates) + '"></follow-panel>'

            document.getElementById("info").innerHTML = innerHTML;
        };
    }
}

customElements.define('route-panel', RoutePanel);