class PointPanel extends HTMLElement {
    constructor(name) {
        super();
        this.name = name;
    }

    connectedCallback() {
        let innerHTML = /*html*/`
            <div id="point_panel" class="absolute w-full max-h-[50%] overflow-auto md:w-[500px] bg-white z-20 bottom-0 rounded-lg">
                <div>Près de ${this.name} <img id="spinner" class="htmx-indicator z-30 bottom-8 mx-auto inset-x-0"
                        src="/pub/bars.svg" />
                </div>

                <div class="flex flex-row justify-center">

                    <md-filled-button id="route_button" >
                    <img slot="icon" src="/pub/directions.png" class="w-4 h-4 mr-1">itinéraire</md-filled-button>
                    <md-filled-button id="cancel_button">annuler</md-filled-button>
                </div>
            </div>
        `;
        this.innerHTML = innerHTML;

        this.querySelector("#route_button").addEventListener("click", () => {
            document.querySelector('vi-main').route();
        });
        this.querySelector("#cancel_button").addEventListener("click", () => {
            document.querySelector('vi-main').clear();
        });
    }
}

export default PointPanel;

customElements.define('vi-point-panel', PointPanel);