class RoutePanel extends HTMLElement {
    constructor() {
        super();
    }

    connectedCallback() {
        const coordinates = JSON.parse(this.getAttribute('coordinates'));
        if (!coordinates || !coordinates[0]) {
            console.error("RoutePanel: coordinates not found");
            return;
        }

        const safeCoordinates = coordinates[0];
        const fastCoordinates = coordinates[1] || null; // Peut être undefined si une seule route
        const viMain = document.querySelector('vi-main');
        let map = viMain.map;

        // Créer le marqueur de destination (bleu) à la fin de la route si pas déjà présent
        if (!viMain.end_marker) {
            const endCoord = safeCoordinates[safeCoordinates.length - 1];
            viMain.end_marker = new maplibregl.Marker({ color: "#00f" }).setLngLat([endCoord[0], endCoord[1]]).addTo(map);
        }

        // Mettre à jour le marqueur de départ (rouge) au début de la route
        if (window.start_marker) {
            const startCoord = safeCoordinates[0];
            window.start_marker.setLngLat([startCoord[0], startCoord[1]]);
            window.start_marker.getElement().style.filter = ''; // Reset color if needed
            window.start_marker.remove();
            window.start_marker = new maplibregl.Marker({ color: "#f00" }).setLngLat([startCoord[0], startCoord[1]]).addTo(map);
        }

        // Mettre à jour l'URL avec les coordonnées de la route
        const startCoord = safeCoordinates[0];
        const endCoord = safeCoordinates[safeCoordinates.length - 1];
        const url = new URL(window.location);
        url.searchParams.set('start_lng', startCoord[0].toFixed(6));
        url.searchParams.set('start_lat', startCoord[1].toFixed(6));
        url.searchParams.set('end_lng', endCoord[0].toFixed(6));
        url.searchParams.set('end_lat', endCoord[1].toFixed(6));
        window.history.replaceState({}, '', url);
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

        // Afficher la route rapide uniquement si elle existe
        if (fastCoordinates) {
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
        } else {
            // Nettoyer la route rapide si elle n'existe pas
            if (map.getLayer("selected_fast")) {
                map.removeLayer("selected_fast");
            }
            if (map.getSource("selected_fast")) {
                map.removeSource("selected_fast");
            }
        }

        viMain.clearDistanceCache();
        let totalDistanceSafe = viMain.calculateTotalDistance(safeCoordinates).toFixed(1);
        viMain.clearDistanceCache();
        let totalDistanceFast = fastCoordinates ? viMain.calculateTotalDistance(fastCoordinates).toFixed(1) : null;
        let totalDurationSafe = totalDistanceSafe / 15.0
        let totalDurationFast = totalDistanceFast ? totalDistanceFast / 15.0 : null;
        let durationStringSafe = "";
        let hours = Math.floor(totalDurationSafe);
        let minutes = Math.round((totalDurationSafe - hours) * 60.0)
        if (hours >= 1.0) {
            durationStringSafe = hours + " heures et "
        }
        durationStringSafe += ` ${minutes} minutes à 15 km/h`
        let durationStringFast = "";
        if (totalDurationFast) {
            hours = Math.floor(totalDurationFast);
            minutes = Math.round((totalDurationFast - hours) * 60.0)
            if (hours >= 1.0) {
                durationStringFast = hours + " heures et "
            }
            durationStringFast += ` ${minutes} minutes à 15 km/h`
        }

        // Construire le bouton rapide seulement s'il existe une route rapide
        const errorText = this.getAttribute('error') ? this.getAttribute('error') : '';
        const fastRouteButton = fastCoordinates ? `
            <md-filled-button id="fast-route-btn"style="--md-sys-color-primary: #ffcbfcff">
                <div style="font-weight: bold;">
                    Itinéraire rapide
                </div>
                <div style="font-size: 0.9em;">
                    Longueur: <span style="font-weight: bold; font-size: 1.3em;">${totalDistanceFast}</span> kms
                </div>
                ${errorText}
            </md-filled-button>
        ` : '';

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
                        ${errorText}
                    </md-filled-button>
                    ${fastRouteButton}
                </div>
                <div style="display: flex; flex-direction: row; justify-content: center; gap: 0.5em; padding-bottom: 1em; position: relative;">
                    <md-outlined-button id="change-start-btn" style="position: absolute; left: 1em; transform: scale(0.85); transform-origin: left center; --md-sys-color-primary: #666666;">changer départ</md-outlined-button>
                    <md-filled-button id="cancel-btn">annuler</md-filled-button>
                </div>
                <div style="display: flex; flex-direction: row; justify-content: center; gap: 1em; padding-bottom: 1em;">
                </div>
            </div>
        `;
        this.innerHTML = innerHTML;

        this.querySelector('#safe-route-btn').addEventListener('click', () => {
            let innerHTML = '<vi-follow-panel route="safe" coordinates="' + JSON.stringify(coordinates) + '"></vi-follow-panel>'
            document.getElementById("info").innerHTML = innerHTML;
        });
        if (this.querySelector('#fast-route-btn')) {
            this.querySelector('#fast-route-btn').addEventListener('click', () => {
                let innerHTML = '<vi-follow-panel route="fast" coordinates="' + JSON.stringify(coordinates) + '"></vi-follow-panel>'
                document.getElementById("info").innerHTML = innerHTML;
            });
        }
        this.querySelector('#change-start-btn').addEventListener('click', () => {
            // Extraire la destination de la dernière coordonnée de la route sécurisée
            const safeCoordinates = coordinates[0];
            const destination = { lng: safeCoordinates[safeCoordinates.length - 1][0], lat: safeCoordinates[safeCoordinates.length - 1][1] };
            // Stocker la destination dans vi-main
            viMain.changeStartDestination = destination;
            let innerHTML = '<vi-change-start></vi-change-start>'
            document.getElementById("info").innerHTML = innerHTML;
        });
        this.querySelector('#cancel-btn').addEventListener('click', () => {
            viMain.clear();
            document.getElementById("info").innerHTML = "";
        });

        var bearing = viMain.calculateBearing(
            safeCoordinates[0][0],
            safeCoordinates[0][1],
            safeCoordinates[safeCoordinates.length - 1][0],
            safeCoordinates[safeCoordinates.length - 1][1]);
        var bounds = viMain.fitBounds(safeCoordinates);
        map.fitBounds(bounds, { bearing, pitch: 0, padding: window.innerHeight * .12, duration: 900 });
    }
}

customElements.define('vi-route-panel', RoutePanel);