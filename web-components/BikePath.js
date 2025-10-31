class BikePath extends HTMLElement {
    constructor() {
        super();
        // Lier 'this' pour que la méthode puisse être utilisée comme un gestionnaire d'événements
        this.fetchBikePaths = this.fetchBikePaths.bind(this);
        this.onMapMove = this.onMapMove.bind(this);
        this.moveTimeout = null;
    }
    connectedCallback() {
        const map = document.querySelector('veloinfo-map').map;

        // S'assurer que la carte est initialisée
        if (map.isStyleLoaded()) {
            this.setupEventListeners(map);
        } else {
            map.once('load', () => this.setupEventListeners(map));
        }
    }

    setupEventListeners(map) {
        this.fetchBikePaths();
        map.on('moveend', this.onMapMove);
    }

    onMapMove() {
        clearTimeout(this.moveTimeout);
        this.moveTimeout = setTimeout(this.fetchBikePaths, 200); // Délai de 200 ms
    }

    async fetchBikePaths() {
        let map = document.querySelector('veloinfo-map').map;
        let bounds = map.getBounds();
        let response = await fetch('/bike_path_geojson', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({
                south: bounds.getSouth(),
                west: bounds.getWest(),
                north: bounds.getNorth(),
                east: bounds.getEast()
            })
        });

        if (response.ok) {
            let data = await response.json();
            const features = data.map(item => {
                const geometry = JSON.parse(item.geojson);
                return {
                    "type": "Feature",
                    "properties": {},
                    "geometry": geometry
                };
            });

            const geojson = {
                "type": "FeatureCollection",
                "features": features
            };

            if (map.getSource('bike_paths')) {
                map.getSource('bike_paths').setData(geojson);
            } else {
                map.addSource('bike_paths', {
                    "type": "geojson",
                    "data": geojson
                });
                map.addLayer({
                    'id': 'bike_paths',
                    'source': 'bike_paths',
                    'type': 'line',
                    "paint": {
                        "line-width": 4,
                        "line-color": "hsl(120, 100%, 40%)",
                        "line-blur": 0,
                        "line-opacity": 0.7
                    }
                });
            }
        }
    }
}
customElements.define('bike-path', BikePath);