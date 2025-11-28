import ViInfo from "./vi-info.js";

export default class ViLayers extends HTMLElement {
    constructor(viMain) {
        super();
        this.viMain = viMain;
        let innerHTML = /*html*/`
            <div id="layers" style="position:absolute;width:500px;max-height:50%;overflow:auto;background:white;z-index:20;bottom:0;border-radius:0.5rem;">
                <div style="display:flex;flex-direction:column;margin:0.5rem;">
                    <div style="display:flex;flex-direction:row;align-items:center;">
                        <md-checkbox style="margin-right:0.5rem;" data-layer="bike_shop"></md-checkbox>
                        <img src="/pub/bike_shop.png" style="height:1.25rem;width:1.25rem;">
                        <div style="margin:0.5rem;">Atelier vélo</div>
                    </div>
                    <div style="display:flex;flex-direction:row;align-items:center;">
                        <md-checkbox style="margin-right:0.5rem;" data-layer="bicycle_repair_station"></md-checkbox>
                        <img src="/pub/bicycle_repair_station.png" style="height:1.25rem;width:1.25rem;">
                        <div style="margin:0.5rem;">Station de réparation autonome</div>
                    </div>
                    <div style="display:flex;flex-direction:row;align-items:center;">
                        <md-checkbox style="margin-right:0.5rem;" data-layer="drinking_water"></md-checkbox>
                        <img src="/pub/drinking_water.png" style="height:1.25rem;width:1.25rem;">
                        <div style="margin:0.5rem;">Eau potable</div>
                    </div>
                    <div style="display:flex;flex-direction:row;align-items:center;">
                        <md-checkbox style="margin-right:0.5rem;" data-layer="bike_parking"></md-checkbox>
                        <img src="/pub/bicycle-parking.png" style="height:1.25rem;width:1.25rem;">
                        <div style="margin:0.5rem;">Parking vélo</div>
                    </div>
                    <div style="display:flex;flex-direction:row;align-items:center;">
                        <md-checkbox style="margin-right:0.5rem;" data-layer="bixi"></md-checkbox>
                        <img src="/pub/bixi.png" style="height:1.25rem;width:1.25rem;">
                        <div style="margin:0.5rem;">Bixi</div>
                    </div>
                    <div style="display:flex;flex-direction:row;border-width:2px;border-style:solid;justify-content:center;">
                        <div style="display:flex;flex-direction:column;">
                        <div style="display:flex;justify-content:center;">largeur:</div>
                        <div style="display:flex;flex-direction:row;align-items:center;">
                            <img src="/pub/cycleway.png">
                            <div style="margin:0.5rem;">Piste cyclable</div>
                        </div>
                        <div style="display:flex;flex-direction:row;align-items:center;">
                            <img src="/pub/bike_lane.png">
                            <div style="margin:0.5rem;">Bande cyclable</div>
                        </div>
                        <div style="display:flex;flex-direction:row;align-items:center;">
                            <img src="/pub/shared_lane.png">
                            <div style="margin:0.5rem;">Partage de la route</div>
                        </div>
                        </div>
                        <div style="display:flex;flex-direction:column;">
                        <div style="display:flex;justify-content:center;">couleur:</div>
                        <div style="display:flex;flex-direction:row;align-items:center;">
                            <div style="height:5px;width:2.5rem;background:#065f46;"></div>
                            <div style="margin:0.5rem;">État normal</div>
                        </div>
                        <div style="display:flex;flex-direction:row;align-items:center;">
                            <div style="height:5px;width:2.5rem;background:#fde047;"></div>
                            <div style="margin:0.5rem;">Problème mineur</div>
                        </div>
                        <div style="display:flex;flex-direction:row;align-items:center;">
                            <div style="height:5px;width:2.5rem;background:#f97316;"></div>
                            <div style="margin:0.5rem;">Piste dangereuse</div>
                        </div>
                        <div style="display:flex;flex-direction:row;align-items:center;">
                            <div style="height:5px;width:2.5rem;background:#b91c1c;"></div>
                            <div style="margin:0.5rem;">Piste Fermée</div>
                        </div>
                        </div>
                    </div>
                    <div style="display:flex;flex-direction:row;align-items:center;">
                        <img src="/pub/road-work.png">
                        <div style="margin:0.5rem;">Travaux</div>
                    </div>

                    <div style="display:flex;justify-content:center;">
                        <md-filled-button id="close_button">fermer</md-filled-button>
                    </div>
                    </div>
                </div>
            </div>`;
        this.innerHTML = innerHTML;
    }

    connectedCallback() {
        let that = this
        this.querySelectorAll('#layers md-checkbox').forEach(input => {
            input.addEventListener('change', function () {
                var layers = localStorage.getItem('layers');
                layers = layers ? JSON.parse(layers) : {};
                const layer = this.dataset.layer;
                if (this.checked) {
                    that.viMain.map.setLayoutProperty(layer, 'visibility', 'visible');
                    layers[layer] = 'visible';
                } else {
                    that.viMain.map.setLayoutProperty(layer, 'visibility', 'none');
                    layers[layer] = 'none';
                }
                localStorage.setItem('layers', JSON.stringify(layers));
            });
        });

        var layers = localStorage.getItem('layers');
        layers = layers ? JSON.parse(layers) : {};
        this.querySelectorAll('#layers md-checkbox').forEach(input => {
            const layer = input.dataset.layer;
            if (layers[layer] === 'visible' || layers[layer] === undefined) {
                input.checked = true;
                that.viMain.map.setLayoutProperty(layer, 'visibility', 'visible');
            } else {
                input.checked = false;
                that.viMain.map.setLayoutProperty(layer, 'visibility', 'none');
            }
        });

        this.querySelector('#close_button').addEventListener('click', async () => {
            let r = await fetch("/info_panel/down");
            let json = await r.json();
            let infoPanel = new ViInfo(json);
            const info = this.viMain.querySelector("#info");
            info.innerHTML = ``;
            info.appendChild(infoPanel);
        });
    }

}

customElements.define('vi-layers', ViLayers);