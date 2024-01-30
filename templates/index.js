getCookie = (name) => {
    let matches = document.cookie.match(new RegExp(
        "(?:^|; )" + name.replace(/([\.$?*|{}\(\)\[\]\\\/\+^])/g, '\\$1') + "=([^;]*)"
    ));
    return matches ? decodeURIComponent(matches[1]) : undefined;
}

if ("serviceWorker" in navigator) {
    navigator.serviceWorker.register("/pub/service-worker.js");
}
var way_ids = "";

var map = new maplibregl.Map({
    container: 'map',
    style: '/style.json',
    center: [getCookie("lng") ? getCookie("lng") : -72.45272261855519, getCookie("lat") ? getCookie("lat") : 45.924806212523265],
    zoom: getCookie("zoom") ? getCookie("zoom") : 6
});

map.on("click", async function (event) {
    select(event);
});

map.on("move", function (e) {
    document.cookie = "zoom=" + map.getZoom();
    document.cookie = "lng=" + map.getCenter().lng;
    document.cookie = "lat=" + map.getCenter().lat;
});

select = async (event) => {
    let width = 20;
    var features = map.queryRenderedFeatures(
        [
            [event.point.x - width / 2, event.point.y - width / 2],
            [event.point.x + width / 2, event.point.y + width / 2]
        ], { layers: ['cycleway', "designated", "shared_lane"] });
    if (!features.length) {
        clear();
        return;
    }
    var feature = features[0];


    var fetch_response = await fetch('/segment/select/' + feature.properties.way_id);
    var response = await fetch_response.json();

    const segment_panel = document.getElementById("select_score");
    if (segment_panel) {
        fetch_response = await fetch('/segment/route/' + feature.properties.way_id + "/" + way_ids);
        response = await fetch_response.json();
        if (response.way_ids.length == 0){
            return;
        }
        way_ids = response.way_ids;
    } else {
        way_ids = feature.properties.way_id;
    }
    display_segment(response.geom, response.way_id);
}

display_segment = async (geom, way_id) => {
    if (map.getLayer("selected")) {
        map.getSource("selected").setData({
            "type": "Feature",
            "properties": {},
            "geometry": {
                "type": "LineString",
                "coordinates": geom
            }
        })
    } else {
        map.addSource("selected", {
            "type": "geojson",
            "data": {
                "type": "Feature",
                "properties": {},
                "geometry": {
                    "type": "LineString",
                    "coordinates": geom
                }
            }
        })
        map.addLayer({
            "id": "selected",
            "type": "line",
            "source": "selected",
            "paint": {
                "line-width": 12,
                "line-color": "#000",
                "line-opacity": 0.3
            }
        });
    }

    if (way_ids) {
        // Display info panel
        var segment_panel = document.getElementById("segment_panel");
        const response = await fetch("/segment_panel/" + way_ids);
        const html = await response.text();
        segment_panel.outerHTML = html;
        // reprocess htmx for the new info panel
        segment_panel = document.getElementById("segment_panel");
        htmx.process(segment_panel);
    }
}

clear = async () => {
    map.getSource("selected").setData({
        "type": "Feature",
        "properties": {},
        "geometry": {
            "type": "LineString",
            "coordinates": []
        }
    })
    // Display info panel
    var segment_panel = document.getElementById("segment_panel");
    const response = await fetch("/segment_panel");
    const html = await response.text();
    segment_panel.outerHTML = html;
    // reprocess htmx for the new info panel
    segment_panel = document.getElementById("segment_panel");
    htmx.process(segment_panel);
}

reset = async () => {
    map.getSource("veloinfo").setUrl("{{martin_url}}/bike_path");
}


