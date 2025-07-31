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

        map.easeTo({
            pitch: 60,
            duration: 800,
        });
        setTimeout(() => {
            window.geolocate.trigger();
            window.geolocate.trackUserLocation = true;
        }, 1000);

        let interval = setInterval(async () => {
            navigator.geolocation.getCurrentPosition((position) => {
                let closestCoordinate = this.findClosestCoordinate(
                    position.coords.longitude,
                    position.coords.latitude,
                    coordinates);
                let totalDistance = window.calculateTotalDistance(coordinates, closestCoordinate).toFixed(1);
                document.getElementById('total_distance').innerText = `${totalDistance} kms`;


                // bearing between current position and last coordinate
                let { latitude, longitude } = position.coords;
                var bearing = calculateBearing(
                    longitude,
                    latitude,
                    coordinates[coordinates.length - 1][0],
                    coordinates[coordinates.length - 1][1]);
                map.easeTo({
                    bearing,
                    duration: 800,
                });
                setTimeout(() => {
                    window.geolocate.trigger();
                }, 1000);
            });
        }, 20000);
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

}

customElements.define("follow-panel", FollowPanel);

