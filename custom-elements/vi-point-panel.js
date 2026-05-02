import { getViMain } from '/custom-elements/vi-context.js';
let html = String.raw;

class PointPanel extends HTMLElement {
    constructor(name, coords) {
        super();
        this.name = name;
        this.coords = coords;
    }

    connectedCallback() {
        let innerHTML = html`
            <div id="point_panel" class="vi-panel">
                <div>Près de ${this.name} <img id="spinner" class="htmx-indicator" style="z-index: 30; bottom: 2rem; margin-left: auto; margin-right: auto; left: 0; right: 0;"
                        src="/pub/bars.svg" />
                </div>

                <div style="display: flex; flex-direction: row; justify-content: center;">

                    <md-filled-button id="route_button" >
                    <img slot="icon" src="/pub/directions.png" style="width: 1rem; height: 1rem; margin-right: 0.25rem;">itinéraire</md-filled-button>
                    <md-filled-button id="cancel_button">annuler</md-filled-button>
                </div>
            </div>
        `;
        this.innerHTML = innerHTML;

        this.querySelector("#route_button").addEventListener("click", () => {
            const viMain = getViMain();
            // Si end_marker n'existe pas, on le crée depuis les coordonnées du panel
            if (!viMain.end_marker && this.coords) {
                viMain.end_marker = new maplibregl.Marker({ color: "#00f" })
                    .setLngLat([this.coords.lng, this.coords.lat])
                    .addTo(viMain.map);
            }
            viMain.route();
        });
        this.querySelector("#cancel_button").addEventListener("click", () => {
            getViMain().clear();
        });
    }
}

export default PointPanel;

customElements.define('vi-point-panel', PointPanel);
