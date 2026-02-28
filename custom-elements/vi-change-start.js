import { getViMain } from '/custom-elements/vi-context.js';

class ChangeStart extends HTMLElement {
    constructor() {
        super();
    }

    connectedCallback() {
        // Récupérer la destination depuis vi-main
        this.destination = getViMain().changeStartDestination;
        let innerHTML = /*html*/`
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
            getViMain().changeStartDestination = null;
            getViMain().clear();
        });
    }
}

customElements.define('vi-change-start', ChangeStart);