class RoutePanel extends HTMLElement {
    constructor() {
        super();
        const coordinates = JSON.parse(this.getAttribute('coordinates'));

        const safeCoordinates = coordinates[0];
        const fastCoordinates = coordinates[1];
        let map = document.querySelector('vi-main').map;
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

        document.querySelector('vi-main').clearDistanceCache();
        let veloinfoMap = document.querySelector('vi-main');
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
                    <md-filled-button id="safe-route-btn" style="--md-sys-color-primary: #c8dff5ff">
                        <div style="font-weight: bold;">
                            Itinéraire sécurisé
                        </div>
                        <div style="font-size: 0.9em;">
                            Longueur: <span style="font-weight: bold; font-size: 1.3em;">${totalDistanceSafe}</span> kms
                        </div>
                        ${this.getAttribute('error')}
                    </md-filled-button>
                    <md-filled-button id="fast-route-btn"style="--md-sys-color-primary: #ffcbfcff">
                        <div style="font-weight: bold;">
                            Itinéraire rapide
                        </div>
                        <div style="font-size: 0.9em;">
                            Longueur: <span style="font-weight: bold; font-size: 1.3em;">${totalDistanceFast}</span> kms
                        </div>
                        ${this.getAttribute('error')}
                    </md-filled-button>
                </div>
                <div style="display: flex; flex-direction: row; justify-content: center; gap: 1em; padding-bottom: 1em;">
                    <md-filled-button id="cancel-btn">annuler</md-filled-button>
                </div>
            </div>
        `;
        this.innerHTML = innerHTML;

        this.querySelector('#safe-route-btn').addEventListener('click', () => {
            let innerHTML = '<vi-follow-panel route="safe" coordinates="' + JSON.stringify(coordinates) + '"></vi-follow-panel>'
            document.getElementById("info").innerHTML = innerHTML;
        });
        this.querySelector('#fast-route-btn').addEventListener('click', () => {
            let innerHTML = '<vi-follow-panel route="fast" coordinates="' + JSON.stringify(coordinates) + '"></vi-follow-panel>'
            document.getElementById("info").innerHTML = innerHTML;
        });
        // this.querySelector('#change-start-btn').addEventListener('click', () => {
        //     let innerHTML = '<change-start></change-start>'
        //     document.getElementById("info").innerHTML = innerHTML;
        // });
        this.querySelector('#cancel-btn').addEventListener('click', () => {
            document.querySelector('vi-main').clear();
            document.getElementById("info").innerHTML = "";
        });

        var bearing = document.querySelector('vi-main').calculateBearing(
            safeCoordinates[0][0],
            safeCoordinates[0][1],
            safeCoordinates[safeCoordinates.length - 1][0],
            safeCoordinates[safeCoordinates.length - 1][1]);
        var bounds = document.querySelector('vi-main').fitBounds(safeCoordinates);
        map.fitBounds(bounds, { bearing, pitch: 0, padding: window.innerHeight * .12, duration: 900 });
    }
}

customElements.define('vi-route-panel', RoutePanel);