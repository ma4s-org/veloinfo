import '@material/web/all.js';
import maplibregl from 'maplibre-gl';
import './web-components/FollowPanel.js';
import './web-components/RoutePanel.js';
import './web-components/RouteSearching.js';
import './web-components/SearchInput.js';
import './web-components/VeloinfoMenu.js';
import './web-components/VeloinfoInstallIos.js';
import './web-components/VeloinfoInstallAndroid.js';
import './web-components/SnowPanel.js';
import htmx from 'htmx.org';

if ("serviceWorker" in navigator) {
    navigator.serviceWorker.register("/pub/service-worker.js");
}

// Set the initial map center and zoom level
// the url parameters take precedence over the cookies
const position = JSON.parse(localStorage.getItem("position"));
var lng = position?.lng || -73.39899762303611;
var lat = position?.lat || 45.921066117828786;
var zoom = position?.zoom || 6;
let params = new URLSearchParams(window.location.search);
if (params.has("lat") && params.has("lng") && params.has("zoom")) {
    lat = parseFloat(params.get("lat"));
    lng = parseFloat(params.get("lng"));
    zoom = parseFloat(params.get("zoom"));
}


// Speed
var speed = 0;
var speed_text = 0;
navigator.geolocation.watchPosition((position) => {
    speed = position.coords.speed * 3.6;
    if (document.getElementById("speed_value")) {
        speed_text = document.getElementById("speed_value").textContent = speed?.toFixed(0) || 0;

        if (speed == 0 || speed == null) {
            document.getElementById("speed_value").parentElement.style.display = "none";
        } else {
            document.getElementById("speed_value").parentElement.style.display = "block";
        }
    }
});

var map = new maplibregl.Map({
    container: 'map',
    style: '/style.json',
    center: [lng, lat],
    zoom: zoom,
    minZoom: 8
});


// Load the layers from the local storage
setTimeout(() => {
    const layers = JSON.parse(localStorage.getItem("layers"));
    if (layers) {
        for (const layer in layers) {
            if (layers[layer] == "visible") {
                map.setLayoutProperty(layer, 'visibility', 'visible');
            } else {
                map.setLayoutProperty(layer, 'visibility', 'none');
            }
        }
    }
}, 1000);

// Load the images
(async () => {
    const bike_image = await map.loadImage('/pub/bicycle-parking.png');
    map.addImage('bike-parking', bike_image.data);
    const drinking_water = await map.loadImage('/pub/drinking_water.png');
    map.addImage('drinking-water', drinking_water.data);
    const bike_shop = await map.loadImage('/pub/bike_shop.png');
    map.addImage('bike-shop', bike_shop.data);
    const bicycle_repair_station = await map.loadImage('/pub/bicycle_repair_station.png');
    map.addImage('bicycle_repair_station', bicycle_repair_station.data);
    const bixi = await map.loadImage('/pub/bixi.png');
    map.addImage('bixi', bixi.data);
    const snow = await map.loadImage('/pub/snow.png');
    map.addImage('snow', snow.data);
})();



map.addControl(new maplibregl.NavigationControl());
let geolocate = new maplibregl.GeolocateControl({
    fitBoundsOptions: {
        maxZoom: 16.5
    },
    positionOptions: {
        enableHighAccuracy: true
    },
    trackUserLocation: true
});
map.addControl(geolocate);

map.on("load", async () => {
    const bounds = map.getBounds();
    htmx.ajax("GET", "/info_panel/up/" + bounds._sw.lng + "/" + bounds._sw.lat + "/" + bounds._ne.lng + "/" + bounds._ne.lat, "#info");
})


map.on("click", async function (event) {
    if (document.getElementById("info_panel_up") ||
        document.getElementById("info_panel_down") ||
        document.getElementById("segment_panel") ||
        document.getElementById("layers") ||
        document.getElementById("point_panel")
    ) {
        select(event);
    }
});

let timeout_url = null;
map.on("move", function (e) {
    if (timeout_url) {
        clearTimeout(timeout_url);
    }
    timeout_url = setTimeout(() => {
        window.history.replaceState({}, "", "/?lat=" + map.getCenter().lat + "&lng=" + map.getCenter().lng + "&zoom=" + map.getZoom());
        const position = {
            "lng": + map.getCenter().lng,
            "lat": + map.getCenter().lat,
            "zoom": + map.getZoom()
        }
        localStorage.setItem("position", JSON.stringify(position));
        if (document.getElementById("info_panel_up")) {
            const bounds = map.getBounds();
            htmx.ajax("GET", "/info_panel/up/" + bounds._sw.lng + "/" + bounds._sw.lat + "/" + bounds._ne.lng + "/" + bounds._ne.lat, "#info");
        }
    }, 1000);

});

let end_marker = null;
async function select(event) {
    if (window.start_marker && end_marker) {
        clear();
    }

    if (window.start_marker && map.getLayer("selected")) {
        selectBigger(event);
        return;
    }

    if (window.start_marker) {
        window.start_marker.remove();
    }
    window.start_marker = new maplibregl.Marker({ color: "#00f" }).setLngLat([event.lngLat.lng, event.lngLat.lat]).addTo(map);

    let width = 20;
    var features = map.queryRenderedFeatures(
        [
            [event.point.x - width / 2, event.point.y - width / 2],
            [event.point.x + width / 2, event.point.y + width / 2]
        ], { layers: ['cycleway', 'designated', 'shared_lane'] });

    if (features.length) {
        var feature = features[0];
        htmx.ajax('GET', '/segment_panel_lng_lat/' + event.lngLat.lng + "/" + event.lngLat.lat, "#info");
    } else {
        const selected = map.getSource("selected");
        if (selected) {
            selected.setData({
                "type": "Feature",
                "properties": {},
                "geometry": {
                    "type": "LineString",
                    "coordinates": []
                }
            });
        }
        htmx.ajax('GET', '/point_panel_lng_lat/' + event.lngLat.lng + "/" + event.lngLat.lat, "#info");
    }
}

async function selectBigger(event) {
    if (end_marker) {
        end_marker.remove();
    }
    end_marker = new maplibregl.Marker({ color: "#f00" }).setLngLat([event.lngLat.lng, event.lngLat.lat]).addTo(map);

    var nodes = await htmx.ajax('GET', '/segment_panel_bigger/' + window.start_marker.getLngLat().lng + "/" + window.start_marker.getLngLat().lat + "/" + event.lngLat.lng + "/" + event.lngLat.lat, "#info");
}



async function clear() {
    console.log("Clearing selection");

    if (window.start_marker) {
        window.start_marker.remove();
        window.start_marker = null;
    }
    if (end_marker) {
        end_marker.remove();
        end_marker = null;
    }
    if (map.getLayer("selected")) {
        map.removeLayer("selected");
    }
    if (map.getSource("selected")) {
        map.removeSource("selected");
    }
    if (map.getLayer("searched_route")) {
        map.removeLayer("searched_route");
    }
    if (map.getSource("searched_route")) {
        map.removeSource("searched_route");
    }

    // Display info panel
    htmx.ajax("GET", "/info_panel/down", "#info");
}

async function route() {
    let info = document.getElementById("info");
    info.innerHTML = `
        <route-searching>
        </route-searching>
    `;
}

function fitBounds(geom) {
    var bounds = geom.reduce((currentBounds, coord) => {
        return [
            [Math.min(coord[0], currentBounds[0][0]), Math.min(coord[1], currentBounds[0][1])], // min coordinates
            [Math.max(coord[0], currentBounds[1][0]), Math.max(coord[1], currentBounds[1][1])]  // max coordinates
        ];
    }, [[Infinity, Infinity], [-Infinity, -Infinity]]);
    return bounds;
}

function calculateBearing(lon1, lat1, lon2, lat2) {
    lon1 = lon1 * Math.PI / 180.0;
    lat1 = lat1 * Math.PI / 180.0;
    lon2 = lon2 * Math.PI / 180.0;
    lat2 = lat2 * Math.PI / 180.0;
    const y = Math.sin(lon2 - lon1) * Math.cos(lat2);
    const x = Math.cos(lat1) * Math.sin(lat2) - Math.sin(lat1) * Math.cos(lat2) * Math.cos(lon2 - lon1);
    let bearing = Math.atan2(y, x) * (180 / Math.PI);
    bearing = (bearing + 360) % 360; // Ensuring the bearing is positive
    return bearing;
} function calculateDistance(lat1, lon1, lat2, lon2) {
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

let distanceCache = {};
function calculateTotalDistance(coordinates, index = 0) {
    if (index in distanceCache) {
        return distanceCache[index];
    }

    let totalDistance = 0;
    for (let i = index; i < coordinates.length - 1; i++) {
        totalDistance += calculateDistance(
            coordinates[i][1], coordinates[i][0],
            coordinates[i + 1][1], coordinates[i + 1][0]
        );
    }

    distanceCache[index] = totalDistance;
    return totalDistance;
}

function clearDistanceCache() {
    distanceCache = {};
}

const ex = { map, clear, route, select, selectBigger, calculateBearing, fitBounds, maplibregl, geolocate, calculateTotalDistance, clearDistanceCache };
Object.assign(window, ex);

export default ex;