import htmx from "htmx.org";

class FollowPanel extends HTMLElement {
    constructor() {
        super();
        let map = document.querySelector('veloinfo-map').map;
        let coordinates = JSON.parse(this.getAttribute('coordinates'));
        let route = this.getAttribute('route');
        if (route === "safe") {
            this.routeCoordinates = coordinates[0];
            if (map.getLayer("selected_fast")) {
                map.removeLayer("selected_fast");
            } if (map.getSource("selected_fast")) {
                map.removeSource("selected_fast");
            }
        } else if (route === "fast") {
            this.routeCoordinates = coordinates[1];
            if (map.getLayer("selected_safe")) {
                map.removeLayer("selected_safe");
            } if (map.getSource("selected_safe")) {
                map.removeSource("selected_safe");
            }
        }

        let totalDistance = document.querySelector('veloinfo-map').calculateTotalDistance(this.routeCoordinates, 0).toFixed(1);
        this.innerHTML = `
            <div class="absolute w-full max-h-[50%] overflow-auto md:w-[500px] bg-white z-20 bottom-0 rounded-lg">
                <div id="follow" style="display: flex; flex-direction: column; justify-content: center;">
                    <div style="display: flex;justify-content: center;">
                        <div>
                            distance à faire :
                        </div>
                        <div id="total_distance" style="margin-left: 2em; font-size: 1.2em; font-weight: bold;">
                            ${totalDistance} kms
                        </div>
                    </div>
                    <div style="display: flex;justify-content: center;">
                        <md-filled-button hx-on:click="document.querySelector('veloinfo-map').clear()" hx-target="#info">annuler</md-filled-button>
                    </div>
                </div>
            </div>
        `;
        htmx.process(this);

    }

    connectedCallback() {
        this.intervalId = setInterval(async () => {
            if (!document.body.contains(this)) {
                clearInterval(this.intervalId);
                return;
            }
            this.updatePosition();
        }, 10_000);
        this.updatePosition();
    }

    disconnectedCallback() {
        if (document.querySelector('veloinfo-map').isGeolocateActive) {
            document.querySelector('veloinfo-map').geolocate.trigger();
        }
        clearInterval(this.intervalId);
    }

    updatePosition() {
        if (this.updating) {
            return;
        }
        navigator.geolocation.getCurrentPosition(async (position) => {

            let closestCoordinate = this.findClosestCoordinate(
                position.coords.longitude,
                position.coords.latitude,
                this.routeCoordinates
            );
            let distanceToClosest = this.calculateDistance(
                position.coords.latitude,
                position.coords.longitude,
                this.routeCoordinates[closestCoordinate][1],
                this.routeCoordinates[closestCoordinate][0]
            );
            console.log("position actuelle: " + position.coords.longitude + ", " + position.coords.latitude);
            console.log("coordonnée la plus proche: " + this.routeCoordinates[closestCoordinate][0] + ", " + this.routeCoordinates[closestCoordinate][1]);
            console.log("distance au point le plus proche: " + distanceToClosest);

            if (distanceToClosest > 0.2) { // 200 meters
                // we are too far from the route. We calculate it again.
                this.updating = true;
                const socket = new WebSocket(`/recalculate_route/${this.getAttribute('route')}/${position.coords.longitude}/${position.coords.latitude}/${this.routeCoordinates[this.routeCoordinates.length - 1][0]}/${this.routeCoordinates[0][this.routeCoordinates[0].length - 1][1]}`);
                socket.onmessage = async (event) => {
                    let data = JSON.parse(event.data);
                    if (data.coordinates) {
                        socket.close();
                        let sourceId = this.getAttribute('route') === "safe" ? "selected_safe" : "selected_fast";
                        document.querySelector('veloinfo-map').map.getSource(sourceId).setData({
                            "type": "Feature",
                            "properties": {},
                            "geometry": {
                                "type": "MultiLineString",
                                "coordinates": [data.coordinates]
                            }
                        });
                        this.routeCoordinates = data.coordinates;
                        this.updating = false;
                        return;
                    } else {
                        console.log(event.data);
                    }
                }
                this.updating = false;
            }
            let totalDistance = document.querySelector('veloinfo-map').calculateTotalDistance(this.routeCoordinates, closestCoordinate).toFixed(1);
            if (document.getElementById('total_distance')) {
                document.getElementById('total_distance').innerText = `${totalDistance} kms`;
            }
        });
        this.setBearing(this.routeCoordinates);
    }

    findClosestCoordinate(longitude, latitude, coordinates) {
        let closestCoordinate = 0;
        let closestDistance = Infinity;
        for (let i = 0; i < coordinates.length; i++) {
            let distance = Math.sqrt(
                Math.pow(longitude - coordinates[i][0], 2) +
                Math.pow(latitude - coordinates[i][1], 2));
            if (distance < closestDistance) {
                closestCoordinate = i;
                closestDistance = distance;
            }
        }
        return closestCoordinate;
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

    setBearing(coordinates) {
        if (!document.body.contains(this)) {
            return;
        }
        navigator.geolocation.getCurrentPosition((position) => {
            let closestCoordinate = this.findClosestCoordinate(
                position.coords.longitude,
                position.coords.latitude,
                coordinates);

            // bearing between current position and 100 meter away
            // Find the first coordinate that is at least 100 meters away from current position (functional style)
            let { latitude, longitude } = position.coords;
            let hundredMeterAwayIndex = coordinates
                .slice(closestCoordinate)
                .findIndex(coord =>
                    this.calculateDistance(
                        latitude,
                        longitude,
                        coord[1],
                        coord[0]
                    ) >= 0.15 // 150 meters
                );
            hundredMeterAwayIndex = hundredMeterAwayIndex === -1
                ? coordinates.length - 1
                : hundredMeterAwayIndex + closestCoordinate;
            var bearing = document.querySelector('veloinfo-map').calculateBearing(
                longitude,
                latitude,
                coordinates[hundredMeterAwayIndex][0],
                coordinates[hundredMeterAwayIndex][1]);
            if (!document.body.contains(this)) {
                return;
            }
            let map = document.querySelector('veloinfo-map').map;
            map.easeTo({
                pitch: 60,
                bearing,
                duration: 1_600,
            });
            setTimeout(() => {
                if (!document.body.contains(this)) {
                    return;
                }
                if (!document.querySelector('veloinfo-map').isGeolocateActive) {
                    document.querySelector('veloinfo-map').geolocate.trigger();
                }
            }, 1_600);
        });
    }

}

customElements.define("follow-panel", FollowPanel);

