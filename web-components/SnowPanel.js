class SnowPanel extends HTMLElement {
    constructor() {
        super();
        this.innerHTML = /*html*/`
            <md-dialog>
                <div slot="headline" id="headline">Neige au sol à <span class="city_name"></span></div>
                <div slot="content">
                    <p>Indiquez si vous avez de la neige au sol sur votre itinéraire.</p>
                </div>
                <div slot="actions">
                    <md-filled-button id="snow_yes">Il y a de la neige au sol</md-filled-button>
                    <md-filled-button id="snow_no">Je ne vois pas de neige</md-filled-button>
                </div>
            </md-dialog>
        `;
    }
    connectedCallback() {
        document.querySelector('#snow_button').addEventListener('click', () => {
            const map = document.querySelector('veloinfo-map').map;

            // Obtenir le point central du canvas de la carte
            const canvas = map.getCanvas();
            const centerPoint = [canvas.width / 2, canvas.height / 2];

            // Récupérer les éléments au centre de la carte sur city
            const dialog = this.querySelector('md-dialog');
            const features = map.queryRenderedFeatures(centerPoint, { layers: ['city'] });
            const cityName = features[0].properties.name;
            dialog.querySelector('.city_name').textContent = cityName;

            dialog.show();

            this.querySelector('#snow_yes').onclick = async () => {
                const features = map.queryRenderedFeatures(centerPoint, { layers: ['city'] });
                const cityName = features[0].properties.name;
                await fetch('/city_snow', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ name: cityName, snow: true })
                })
                dialog.close();

                document.querySelector('veloinfo-map').insertCitySnow();
            };

            this.querySelector('#snow_no').onclick = async () => {
                const features = map.queryRenderedFeatures(centerPoint, { layers: ['city'] });
                const cityName = features[0].properties.name;
                await fetch('/city_snow', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ name: cityName, snow: false })
                });
                dialog.close();
                document.querySelector('veloinfo-map').insertCitySnow();
            };

        });
    }
}


customElements.define('snow-panel', SnowPanel);