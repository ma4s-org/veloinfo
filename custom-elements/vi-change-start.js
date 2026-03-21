import { getViMain } from '/custom-elements/vi-context.js';

let html = String.raw;

class ChangeStart extends HTMLElement {
    constructor() {
        super();
    }

    connectedCallback() {
        // Récupérer la destination depuis vi-main
        const viMain = getViMain();
        this.destination = viMain.changeStartDestination;
        let innerHTML = html`
            <div class="vi-panel">
                <div style="padding: 1rem;">
                    <h2 style="font-size: 1.25rem; font-weight: 700;">
                        Changer le point de départ
                    </h2>
                    <p>
                        Veuillez sélectionner un nouveau point de départ sur la carte.
                    </p>
                </div>
                <div style="display: flex; justify-content: center;">
                        <md-filled-button id="cancel-btn">
                            annuler
                        </md-filled-button>
                </div>
            </div>
        `;
        this.innerHTML = innerHTML;

        this.querySelector('#cancel-btn').addEventListener('click', () => {
            viMain.changeStartDestination = null;
            viMain.clear();
        });

        // Écouter le clic sur la carte pour sélectionner le nouveau départ
        const handleMapClick = (event) => {
            viMain.setMarkers(event.lngLat.lng, event.lngLat.lat, this.destination.lng, this.destination.lat);
            viMain.changeStartDestination = null;
            viMain.map.off("click", handleMapClick);
            viMain.route();
        };
        viMain.map.on("click", handleMapClick);
    }
}

customElements.define('vi-change-start', ChangeStart);
