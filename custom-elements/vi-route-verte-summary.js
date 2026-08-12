import { getViMain } from '/custom-elements/vi-context.js';

let html = String.raw;

const REFRESH_DEBOUNCE_MS = 300;

function fmtKm(km) {
    if (km == null || Number.isNaN(km)) return '0,0';
    // Locale fr-CA utilise la virgule comme séparateur décimal.
    return km.toLocaleString('fr-CA', { minimumFractionDigits: 1, maximumFractionDigits: 1 });
}

class ViRouteVerteSummary extends HTMLElement {
    constructor() {
        super();
        this._moveTimeout = null;
        this._total = null;
    }

    connectedCallback() {
        this.render();
        // Premier chargement : récupérer total + visible
        this.refresh(true);
        // S'abonner aux déplacements de carte pour rafraîchir la portion visible
        const map = getViMain()?.map;
        if (map) {
            this._onMove = () => {
                if (this._moveTimeout) clearTimeout(this._moveTimeout);
                this._moveTimeout = setTimeout(() => this.refresh(false), REFRESH_DEBOUNCE_MS);
            };
            map.on('moveend', this._onMove);
        }
    }

    disconnectedCallback() {
        const map = getViMain()?.map;
        if (map && this._onMove) map.off('moveend', this._onMove);
        if (this._moveTimeout) clearTimeout(this._moveTimeout);
    }

    async refresh(includeTotal) {
        const map = getViMain()?.map;
        if (!map) return;
        const bounds = map.getBounds();
        const url = `/route_verte/stats/${bounds._sw.lng}/${bounds._sw.lat}/${bounds._ne.lng}/${bounds._ne.lat}`;
        try {
            const r = await fetch(url);
            if (!r.ok) return;
            const json = await r.json();
            if (includeTotal) this._total = json.total;
            this._visible = json.visible;
            this.update();
        } catch (e) {
            console.warn('route_verte_stats:', e);
        }
    }

    update() {
        const total = this._total || { with_infra: 0, without_infra: 0 };
        const visible = this._visible || { with_infra: 0, without_infra: 0 };
        const totalAll = (total.with_infra || 0) + (total.without_infra || 0);
        const visibleAll = (visible.with_infra || 0) + (visible.without_infra || 0);
        const visWithEl = this.querySelector('#rv_vis_with');
        const visWithoutEl = this.querySelector('#rv_vis_without');
        const visAllEl = this.querySelector('#rv_vis_all');
        const totWithEl = this.querySelector('#rv_tot_with');
        const totWithoutEl = this.querySelector('#rv_tot_without');
        const totAllEl = this.querySelector('#rv_total');
        if (visWithEl) visWithEl.textContent = fmtKm(visible.with_infra);
        if (visWithoutEl) visWithoutEl.textContent = fmtKm(visible.without_infra);
        if (visAllEl) visAllEl.textContent = fmtKm(visibleAll);
        if (totWithEl) totWithEl.textContent = fmtKm(total.with_infra);
        if (totWithoutEl) totWithoutEl.textContent = fmtKm(total.without_infra);
        if (totAllEl) totAllEl.textContent = fmtKm(totalAll);
    }

    render() {
        this.innerHTML = html`
            <div style="position: absolute; top: 250px; right: 6px; z-index: 10;
                        background-color: white; border: 1px solid rgb(209 213 219);
                        border-radius: 0.375rem; padding: 0.4rem 0.5rem;
                        font-size: small; line-height: 1.3; color: #111827;
                        box-shadow: 0 1px 2px rgba(0,0,0,0.05);
                        min-width: 10rem; max-width: 14rem;">
                <div style="font-weight: bold; margin-bottom: 0.15rem;">
                    Route verte — visible
                </div>
                <div style="display: flex; justify-content: space-between; gap: 0.5rem; padding-left: 0.4rem;">
                    <span>Aménagée</span>
                    <span><b id="rv_vis_with">0,0</b> km</span>
                </div>
                <div style="display: flex; justify-content: space-between; gap: 0.5rem; padding-left: 0.4rem;">
                    <span>Non amén.</span>
                    <span><b id="rv_vis_without">0,0</b> km</span>
                </div>
                <div style="display: flex; justify-content: space-between; gap: 0.5rem;">
                    <span><b>Total visible</b></span>
                    <span><b id="rv_vis_all">0,0</b> km</span>
                </div>
                <hr style="border: none; border-top: 1px solid #e5e7eb; margin: 0.25rem 0;">
                <div style="font-weight: bold; margin-bottom: 0.15rem;">
                    Réseau total
                </div>
                <div style="display: flex; justify-content: space-between; gap: 0.5rem; padding-left: 0.4rem;">
                    <span>Aménagée</span>
                    <span><b id="rv_tot_with">0,0</b> km</span>
                </div>
                <div style="display: flex; justify-content: space-between; gap: 0.5rem; padding-left: 0.4rem;">
                    <span>Non amén.</span>
                    <span><b id="rv_tot_without">0,0</b> km</span>
                </div>
                <div style="display: flex; justify-content: space-between; gap: 0.5rem;">
                    <span><b>Total</b></span>
                    <span><b id="rv_total">0,0</b> km</span>
                </div>
            </div>
        `;
    }
}

customElements.define('vi-route-verte-summary', ViRouteVerteSummary);
export default ViRouteVerteSummary;