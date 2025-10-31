class PointPanel extends HTMLElement {
    constructor() {
        super();
    }

    connectedCallback() {
        this.innerHTML = /*html*/`
            <div id="point_panel" class="absolute w-full max-h-[50%] overflow-auto md:w-[500px] bg-white z-20 bottom-0 rounded-lg">
                <div>Près de ${this.getAttribute('name')} <img id="spinner" class="htmx-indicator z-30 bottom-8 mx-auto inset-x-0"
                        src="/pub/bars.svg" />
                </div>

                <div class="flex flex-row justify-center">

                    <md-filled-button id="route_button" hx-on:click="document.querySelector('veloinfo-map').route()"><img slot="icon"
                            src="/pub/directions.png" class="w-4 h-4 mr-1">itinéraire</md-filled-button>
                    <md-filled-button hx-on:click="document.querySelector('veloinfo-map').clear()"
                        hx-target="#info">annuler</md-filled-button>
                </div>
            </div>
        `;
    }
}

customElements.define('point-panel', PointPanel);