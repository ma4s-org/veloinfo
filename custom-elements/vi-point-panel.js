import { getViMain } from '/custom-elements/vi-context.js';
let html = String.raw;

class PointPanel extends HTMLElement {
    constructor(name, coords, way_ids = null, on_cycleway = false) {
        super();
        this.name = name;
        this.coords = coords;
        this.way_ids = way_ids;
        this.on_cycleway = on_cycleway;
    }

    connectedCallback() {
        let innerHTML = html`
            <div id="point_panel" class="vi-panel">
                <div>
                    <div style="font-size: small; font-weight: bold;">${this.name || ""}</div>
                </div>

                <div style="display: flex; flex-direction: row; justify-content: center; margin-top: 0.5rem;">
                    <md-filled-button id="route_button" >
                    <img slot="icon" src="/pub/directions.png" style="width: 1rem; height: 1rem; margin-right: 0.25rem;">itinéraire</md-filled-button>
                    <md-filled-button id="report_button">
                    <span slot="icon" style="font-size: 1rem; margin-right: 0.25rem;">⚠️</span>signaler</md-filled-button>
                </div>
                <div style="display: flex; flex-direction: row; justify-content: center; margin-top: 0.5rem;">
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
        
        if (this.querySelector("#report_button")) {
            this.querySelector("#report_button").addEventListener("click", async () => {
                const viMain = getViMain();
                
                // Supprimer le point_panel avant de passer en mode signalement
                const pointPanel = document.getElementById("point_panel");
                if (pointPanel) {
                    pointPanel.parentElement.innerHTML = "";
                }
                
                // Nettoyer les anciens markers
                if (viMain.start_marker) {
                    viMain.start_marker.remove();
                    viMain.start_marker = null;
                }
                if (viMain.end_marker) {
                    viMain.end_marker.remove();
                    viMain.end_marker = null;
                }
                
                // Supprimer seulement les layers/sources de sélection
                ["selected", "selected-outline"].forEach(layer => {
                    if (viMain.map.getLayer(layer)) viMain.map.removeLayer(layer);
                });
                ["selected", "selected-outline"].forEach(source => {
                    if (viMain.map.getSource(source)) viMain.map.removeSource(source);
                });
                
                // Activer le mode "signaler"
                viMain._isReporting = true;
                
                // Sauvegarder le point de départ
                const lng = this.coords.lng;
                const lat = this.coords.lat;
                viMain._reportingSegment = { startLng: lng, startLat: lat };
                
                // Simuler un double-clic au MÊME endroit
                viMain._firstClick = { lng, lat };
                
                // Créer un événement factice pour le 2ème clic (au même endroit)
                const fakeEvent = {
                    lngLat: { lng, lat }
                };
                
                // Appeler select() qui va détecter _firstClick et créer le cercle + segment_panel
                await viMain.select(fakeEvent);
            });
        }
        
        this.querySelector("#cancel_button").addEventListener("click", () => {
            getViMain().clear();
        });
    }
}

export default PointPanel;

customElements.define('vi-point-panel', PointPanel);
