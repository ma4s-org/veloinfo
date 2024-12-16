class FollowPanel extends HTMLElement {
    constructor() {
        super();
        let totalDistance = window.calculateTotalDistance(window.coordinates, 0);
        this.innerHTML = `
            <div class="absolute w-full max-h-[50%] overflow-auto md:w-[500px] bg-white z-20 bottom-0 rounded-lg">
                <div id="follow" style="display: flex; flex-direction: column; justify-content: center;">
                    <div style="display: flex;justify-content: center;">
                        <div>
                            distance à faire :
                        </div>
                        <div style="margin-left: 2em; font-weight: bold;">
                            ${totalDistance.toFixed(2)} kms
                        </div>
                    </div>
                    <div>
                        ${this.getAttribute('error')}
                    </div>
                    <div style="display: flex;justify-content: center;">
                        <md-filled-button hx-on:click="clear()" hx-target="#info">annuler</md-filled-button>
                    </div>
                </div>
            </div>
        `;

        map.easeTo({
            pitch: 60,
            duration: 800,
        });
        setTimeout(() => {
            window.geolocate.trigger();
            window.geolocate.trackUserLocation = true;
        }, 1000);

        let interval = setInterval(() => {
            if (!document.getElementById('follow')) {
                clearInterval(interval);
                return;
            }
            navigator.geolocation.getCurrentPosition((position) => {
                let closestCoordinate = this.findClosestCoordinate(
                    position.coords.longitude,
                    position.coords.latitude,
                    window.coordinates);
                let totalDistance = window.calculateTotalDistance(window.coordinates, closestCoordinate);
                document.getElementById('total_distance').innerText = `${totalDistance.toFixed(2)} kms`;


                // bearing between current position and last coordinate
                let { latitude, longitude } = position.coords;
                var bearing = calculateBearing(
                    longitude,
                    latitude,
                    window.coordinates[window.coordinates.length - 1][0],
                    window.coordinates[window.coordinates.length - 1][1]);
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

