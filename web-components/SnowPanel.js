class SnowPanel extends HTMLElement {
    constructor() {
        super();
        this.innerHTML = `
            <style>
            snow-panel {
                display: flex;
                margin: 10px 0;
                flex-direction: row;
                justify-content: center;
                outline: 1px solid #e5e7eb;
                cursor: pointer;
                z-index: 10;
            }
            snow-panel::before, ::after {
                box-sizing: border-box;
                border-width: 0;
                border-style: solid;
                border-color: #e5e7eb;
            }
            snow-panel #snow{
                position: relative;
                display: flex;
                flex-direction: column;
                border-radius: 0.375rem;
                background-color: #fff;
                justify-content: center;
                align-items: center;
                width: 32px;
                height: 32px;
            }
            snow-panel .snow{
                padding: .5em;
                font-size: 1rem;
            }
            </style>


            <div id="snow" class="snow">
                <img src="/pub/snow-icone.png" alt="Snow" style="width: 18px; height: 18px">
            </div>

            <md-dialog>
                <div slot="headline" id="headline">Neige</div>
            </md-dialog>
        `;
    }
    connectedCallback() {
        this.querySelector("#snow").addEventListener("click", () => this.showDialog());
    }

    async showDialog() {
        this.querySelector("md-dialog").show();
        const urlParams = new URLSearchParams(window.location.search);
        this.lng = urlParams.get('lng');
        this.lat = urlParams.get('lat');
        const response = await fetch(`city_snow/${this.lng}/${this.lat}`);
        const data = await response.json();
        console.log(data);
        this.querySelector("#headline").innerHTML = `Neige au sol à ${data.name}`;

    }
}


customElements.define('snow-panel', SnowPanel, {});