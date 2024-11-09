if ("serviceWorker" in navigator) {
    navigator.serviceWorker.register("/pub/service-worker.js");
}

(async () => {
    try {
        const wakeLock = await navigator.wakeLock.request("screen");
    } catch (err) {
        // the wake lock request fails - usually system related, such being low on battery
        console.log(`${err.name}, ${err.message}`);
    }
})();

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

// keep the screen open
setInterval(() => {
    try {
        navigator.wakeLock.request("screen");
    } catch (err) {
        // the wake lock request fails - usually system related, such being low on battery
        console.log(`${err.name}, ${err.message}`);
    }
}, 30000);

// Speed
navigator.geolocation.watchPosition((position) => {
    speed = position.coords.speed * 3.6;
    speed_text = document.getElementById("speed_value").textContent = speed?.toFixed(0) || 0;

    if (speed == 0 || speed == null) {
        document.getElementById("speed_value").parentElement.style.display = "none";
    } else {
        document.getElementById("speed_value").parentElement.style.display = "block";
    }
});

var map = new maplibregl.Map({
    container: 'map',
    style: '/style.json',
    center: [lng, lat],
    zoom: zoom,
    minZoom: 8
});
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

let start_marker = null;
let end_marker = null;
async function select(event) {
    if (start_marker && map.getLayer("selected")) {
        selectBigger(event);
        return;
    }

    if (start_marker) {
        start_marker.remove();
    }
    start_marker = new maplibregl.Marker({ color: "#00f" }).setLngLat([event.lngLat.lng, event.lngLat.lat]).addTo(map);

    let width = 20;
    var features = map.queryRenderedFeatures(
        [
            [event.point.x - width / 2, event.point.y - width / 2],
            [event.point.x + width / 2, event.point.y + width / 2]
        ], { layers: ['cycleway'] });

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

    var nodes = await htmx.ajax('GET', '/segment_panel_bigger/' + start_marker.getLngLat().lng + "/" + start_marker.getLngLat().lat + "/" + event.lngLat.lng + "/" + event.lngLat.lat, "#info");
}



async function clear() {
    if (start_marker) {
        start_marker.remove();
        start_marker = null;
    }
    if (end_marker) {
        end_marker.remove();
        end_marker = null;
    }
    map.removeLayer("selected");
    map.removeSource("selected");

    // Display info panel
    htmx.ajax("GET", "/info_panel/down", "#info");
}

async function route() {
    const button = document.getElementById("route_button");
    button.classList.add("htmx-request");
    var end = start_marker.getLngLat();
    // get the position of the device
    var start = await new Promise((resolve, reject) => {
        navigator.geolocation.getCurrentPosition((position) => {
            resolve(position);
        });
    });
    await htmx.ajax("GET", "/route/" + start.coords.longitude + "/" + start.coords.latitude + "/" + end.lng + "/" + end.lat, "#info");
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
}