import htmx from "htmx.org";

class FollowPanel extends HTMLElement {
    constructor() {
        super();
        let coordinates = JSON.parse(this.getAttribute('coordinates'));
        let totalDistance = window.calculateTotalDistance(coordinates, 0).toFixed(1);
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
                        <md-filled-button hx-on:click="clear()" hx-target="#info">annuler</md-filled-button>
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
        }, 20_000);
        this.updatePosition();
    }

    disconnectCallback() {
        clearInterval(this.intervalId);
    }

    updatePosition(){
        if (this.updating) {
            return;
        }
        let coordinates = JSON.parse(this.getAttribute('coordinates'));            
        navigator.geolocation.getCurrentPosition(async (position) => {

            let closestCoordinate = this.findClosestCoordinate(
                position.coords.longitude,
                position.coords.latitude,
                coordinates
            );
            let distanceToClosest = this.calculateDistance(
                position.coords.latitude,
                position.coords.longitude,
                coordinates[closestCoordinate][1],
                coordinates[closestCoordinate][0]
            );
            if (distanceToClosest > .2) { // 200 meters
                // we are too far from the route. We calculate it again.
                this.updating = true;
                if (!map.getSource("searched_route2")) {
                    map.addSource("searched_route2", {
                        "type": "geojson",
                        "data": {
                            "type": "Feature",
                            "properties": {},
                            "geometry": {
                                "type": "MultiLineString",
                                "coordinates": []
                            }
                        }
                    });
                    map.addLayer({
                        'id': 'searched_route2',
                        'source': 'searched_route2',
                        'type': 'line',
                        "paint": {
                            "line-width": 8,
                            "line-color": "hsla(186, 45%, 61%, 1.00)",
                            "line-blur": 0,
                            "line-opacity": 0.50
                        }
                    });
                }
                const socket = new WebSocket(`/recalculate_route/${position.coords.longitude}/${position.coords.latitude}/${coordinates[coordinates.length - 1][0]}/${coordinates[coordinates.length - 1][1]}`);
                let coordinates2 = [];
                socket.onmessage = async (event) => {                    
                    if (event.data.startsWith("{\"coordinates\"")) {
                        let coordinates = JSON.parse(event.data).coordinates;
                        socket.close();
                        if (map.getSource("searched_route2") != null) {
                            map.removeLayer("searched_route2");
                            map.removeSource("searched_route2");
                        }
                        map.getSource("selected").setData({
                            "type": "Feature",
                            "properties": {},
                            "geometry": {
                                "type": "MultiLineString",
                                "coordinates": [coordinates]
                            }
                        });
                        this.setAttribute('coordinates', JSON.stringify(coordinates));
                        this.updating = false;
                        return;
                    } else{
                        console.log(event.data);
                    }
                }
                this.updating = false;
            }
            let totalDistance = window.calculateTotalDistance(coordinates, closestCoordinate).toFixed(1);
            if (document.getElementById('total_distance')){
                document.getElementById('total_distance').innerText = `${totalDistance} kms`;
            }
        });
        this.setBearing(coordinates);
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
            var bearing = calculateBearing(
                longitude,
                latitude,
                coordinates[hundredMeterAwayIndex][0],
                coordinates[hundredMeterAwayIndex][1]);
            map.easeTo({
                bearing,
                duration: 1_600,
            });
            setTimeout(() => {
                map.easeTo({
                    pitch: 60,
                    duration: 1_600,
                });
                setTimeout(() => {
                    if (!window.isGeolocateActive){
                        window.geolocate.trigger();
                    }
                }, 2_000);
            }, 2_000);
        });
    }

}

customElements.define("follow-panel", FollowPanel);

