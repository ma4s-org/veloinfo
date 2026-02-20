import { getViMain } from '/custom-elements/vi-context.js';

class ChangeStart extends HTMLElement {
    constructor() {
        super();
    }

    connectedCallback() {
        // Récupérer la destination depuis vi-main
        this.destination = getViMain().changeStartDestination;
        let innerHTML = /*html*/`
            <div class="absolute w-full max-h-[50%] overflow-auto md:w-[500px] bg-white z-20 bottom-0 rounded-lg">
                <div class="p-4">
                    <h2 class="text-xl font-bold">
                        Changer le point de départ
                    </h2>
                    <p>
                        Veuillez sélectionner un nouveau point de départ sur la carte.
                    </p>
                </div>
                <div class="flex justify-center">
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