import ViPhotoScroll from "./vi-photo-scroll.js";
import { getViMain } from '/custom-elements/vi-context.js';
import ViInfo from "./vi-info.js";
import { showReplyForm } from '/custom-elements/vi-reply-form.js';
let html = String.raw;

class SegmentPanel extends HTMLElement {
    constructor(data) {
        super();
        this.data = data;
    }

    connectedCallback() {
        // Initialiser le panneau avec les données
        if (this.data) {
            this.render(this.data);
        }
    }
    
    // Mettre à jour le segment avec une nouvelle géométrie
    updateSegment(newData) {
        // Mettre à jour les données
        if (newData.geom_json) {
            this.data.geom_json = newData.geom_json;
            // Mettre à jour l'input caché geom_json dans le formulaire
            const geomInput = this.querySelector('input[name="geom_json"]');
            if (geomInput) {
                geomInput.value = newData.geom_json;
            }
        }
    }

    render(data) {
        let photo_ids = data.photo_ids;
        let html = String.raw;
        let photos = photo_ids ? photo_ids.map(id => html`
            <img  style="height: 6rem; border-radius: 0.375rem; padding: 0.5rem; cursor: pointer;" src="/images/${id}_thumbnail.jpeg" alt="photo"
                hx-get="/photo_scroll/${id}/${this.getAttribute('way_ids')}" hx-target="#photo_scroll">
        `).join('') : '';

        let inner = '';
        if (data.edit) {
            inner = html`
                <form>
                    <input type="hidden" name="way_ids" value="${data.way_ids}">
                    <input type="hidden" name="geom_json" value='${data.geom_json || ""}'>
                    <input type="text" name="user_name" style="border: 2px solid; border-color: #80808099;" placeholder="Nom" value="${data.user_name}">
                    <textarea rows="4" cols="50" name="comment" style="border: 2px solid; border-color: #80808099;" placeholder="Commentaire"></textarea>
                    <div style="text-transform: uppercase; margin: 0.5rem;">
                        <label for="photo">Choisissez une photo :</label>
                        <input type="file" id="photo" name="photo">
                    </div>
                    <div style="display: flex; justify-content: center;">
                        <md-filled-button id="save" type="button">Enregistrer</md-filled-button>
                        <md-filled-button id="cancel" type="button">annuler</md-filled-button>  
                    </div>
                </form>
            `;
        } else {
            inner = html`
                <div>
                    <div style="display: flex; margin-bottom: 0.5rem; margin-top: 0.5rem;">
                        <div>
                            <div style="font-size: small; font-weight: bold;">${data.segment_name}</div>
                            <div style="font-size: small; color: gray;">${data.comment}</div>
                        </div>
                    </div>
                    <div class="">
                        <div style="display: flex; flex-direction: row; justify-content: center;">
                            Choisissez un second point pour aggrandir la sélection ou
                        </div>
                        <div style="display: flex; flex-direction: row; justify-content: center;">
                            <md-filled-button id="edit_md"><img slot="icon"
                                    src="/pub/edit.png" style="width: 1rem; height: 1rem; margin-right: 0.25rem;">
                                Modifier</md-filled-button>
                        </div>
                        <div style="display: flex; flex-direction: row; justify-content: center; padding: 0.5rem;">
                            <md-filled-button
                                id="cancel">annuler</md-filled-button>
                        </div>
                    </div>
                </div>
            `;
        }

        let innerHTML = html`
            <div id="segment_panel" style="position: absolute; width: 100%; max-height: 50%; overflow: auto; max-width: 500px; background-color: white; z-index: 20; bottom: 0; border-radius: 0.5rem;">
                <img id="spinner" style="z-index: 30; bottom: 2rem; margin-left: auto; margin-right: auto; left: 0; right: 0;" class="htmx-indicator" src="/pub/bars.svg" />
                <div  style="padding: 0.5rem; margin: 0.25rem;">
                    ${inner}
                    <div style="display: flex; flex-direction: row; overflow: auto;">
                        ${data.photo_ids.map(photo_id => /*html*/`
                            <img id="${photo_id}" class="photo-thumbnail" style="height: 6rem; cursor: pointer;"
                                    src="/images/${photo_id}_thumbnail.jpeg" alt="photo">
                        `).join('')}
                    </div>
                    <div id="photo_scroll"></div>
                    <div style="text-transform: uppercase; margin: 0.5rem;">historique</div>
                    <div style="overflow: auto; max-height: 12rem; md:height: 500px;">
                        <hr>
                        ${(data.contributions || data.history || []).map(contribution => html`
                            <div style="padding: 0.5rem; border-bottom: 1px solid #e5e7eb;">
                                <infopanel-contribution
                                    comment-id="${contribution.id || ''}"
                                    created_at="${contribution.created_at}"
                                    timeago="${contribution.timeago}"
                                    name="${contribution.name}"
                                    photo_path_thumbnail="${contribution.photo_path_thumbnail}"
                                    report_id="${contribution.report_id}"
                                    user_name="${contribution.user_name}"
                                    timestamp="${contribution.timestamp}"
                                    comment="${contribution.comment}">
                                </infopanel-contribution>
                            </div>
                        `).join('')}
                    </div>
                </div>
            </div>
        `;
        this.innerHTML = innerHTML;
        htmx.process(this);

        let that = this;

        this.querySelectorAll('.photo-thumbnail').forEach(img => {
            img.addEventListener('click', async (event) => {
                let photo_id = event.target.id;
                let response = await fetch(`/photo_scroll/${photo_id}/${data.way_ids}`);
                let photoScrollData = await response.json();
                let photoScroll = new ViPhotoScroll(photoScrollData);
                document.querySelector('#photo_scroll_inner')?.remove();
                this.querySelector('#photo_scroll').innerHTML = '';
                this.querySelector('#photo_scroll').appendChild(photoScroll);
            });
        });

        this.querySelector('#save')?.addEventListener('click', async (event) => {
            that.querySelector('#save').disabled = true;
            // clear the cache first
            const cacheNames = await caches.keys();
            Promise.all(
                cacheNames.map(name => caches.delete(name))
            );
            await fetch('/segment_panel', {
                method: 'POST',
                body: new FormData(this.querySelector('form'))
            });
            event.preventDefault();
            // Après un nouveau report, basculer vers le panel de contributions
            getViMain().infoPanelUp();
        });
        this.querySelector('#cancel')?.addEventListener('click', async (event) => {
            getViMain().clear();
            event.preventDefault();
        });

        if (!data.edit) {
            this.querySelector('#route_md')?.remove();
            const editBtn = this.querySelector('#edit_md');
            if (editBtn) {
                editBtn.onclick = async () => {
                    // Mode édition : recréer le panel avec edit=true, sans appel serveur
                    const editData = {
                        ...data,
                        edit: true
                    };
                    let segment_panel = new SegmentPanel(editData);
                    document.querySelector('#info').innerHTML = '';
                    document.querySelector('#info').appendChild(segment_panel);
                };
            }
        }

        let map = getViMain().map;
        
        // Si geom_json est vide, on ne peut pas parser
        if (!data.geom_json || data.geom_json.trim() === "") {
            console.warn("SegmentPanel: geom_json est vide, affichage sans géométrie");
            return;
        }
        
        var geom = JSON.parse(data.geom_json);

        // Déterminer le type de géométrie et l'adapter
        let geomType = "MultiLineString";
        let geomCoords = geom;
        
        // Si le geom est déjà un objet GeoJSON complet (Polygon, MultiPolygon, etc.)
        if (geom.type && geom.coordinates) {
            geomType = geom.type;
            geomCoords = geom.coordinates;
        }

        if (data.fit_bounds) {
            // Adapter fitBounds selon le type de géométrie
            let coordsToUse = geomCoords;
            // Pour MultiPolygon: [[[[x,y],...],...],...], on prend le premier polygon's outer ring
            if (geomType === "MultiPolygon" && Array.isArray(geomCoords[0])) {
                coordsToUse = geomCoords[0][0];
            }
            // Pour Polygon: [[[x,y],...],...], on prend le outer ring (premier élément)
            if (geomType === "Polygon" && Array.isArray(geomCoords[0])) {
                coordsToUse = geomCoords[0];
            }
            // Pour LineString: [[lng,lat], [lng,lat], ...] - déjà dans le bon format
            if (geomType === "LineString") {
                coordsToUse = geomCoords;
            }
            // Pour MultiLineString: [[[lng,lat],...],...], on prend le premier linestring
            if (geomType === "MultiLineString" && Array.isArray(geomCoords[0])) {
                coordsToUse = geomCoords[0];
            }
            // Pour Polygon dans le cas de report (géométrie fermée), on utilise tous les points
            if (geomType === "Polygon") {
                coordsToUse = geomCoords[0];
            }
            
            // Calculer les bounds à partir des coordonnées
            const bounds = coordsToUse.reduce(
                (bounds, coord) => {
                    return [
                        [Math.min(coord[0], bounds[0][0]), Math.min(coord[1], bounds[0][1])],
                        [Math.max(coord[0], bounds[1][0]), Math.max(coord[1], bounds[1][1])]
                    ];
                },
                [[Infinity, Infinity], [-Infinity, -Infinity]]
            );
            map.fitBounds(bounds, { padding: window.innerHeight * .12 });
        }
        const viMain = getViMain();
        if (!data.edit) {
            // Nettoyer les couches existantes (selected et selected-outline)
            if (map.getLayer("selected")) {
                map.removeLayer("selected");
            }
            if (map.getLayer("selected-outline")) {
                map.removeLayer("selected-outline");
            }
            if (map.getSource("selected")) {
                map.removeSource("selected");
            }
            
            // Créer la nouvelle source
            map.addSource("selected", {
                "type": "geojson",
                "data": {
                    "type": "Feature",
                    "properties": {},
                    "geometry": {
                        "type": geomType,
                        "coordinates": geomCoords
                    }
                }
            });
            
            // Choisir le type de layer selon la géométrie
            if (geomType === "Polygon" || geomType === "MultiPolygon") {
                // Layer de remplissage pour les polygones
                map.addLayer({
                    "id": "selected",
                    "type": "fill",
                    "source": "selected",
                    "paint": {
                        "fill-color": "#0000ff",
                        "fill-opacity": 0.3,
                        "fill-antialias": true
                    }
                }, "Road labels");
                
                // Ajouter aussi un contour pour mieux voir les bords
                map.addLayer({
                    "id": "selected-outline",
                    "type": "line",
                    "source": "selected",
                    "paint": {
                        "line-width": 2,
                        "line-color": "#0000ff",
                        "line-opacity": 0.8
                    }
                }, "Road labels");
            } else {
                // Layer de ligne pour LineString/MultiLineString
                map.addLayer({
                    "id": "selected",
                    "type": "line",
                    "source": "selected",
                    "paint": {
                        "line-width": 50,
                        "line-color": "hsl(205, 100%, 50%)",
                        "line-blur": 0,
                        "line-opacity": 0.50
                    }
                }, "Road labels");
            }
        }
    }
}
export default SegmentPanel;
customElements.define('vi-segment-panel', SegmentPanel);

class InfopanelContribution extends HTMLElement {
    constructor() {
        super();
    }

    connectedCallback() {
        const comment_id = this.getAttribute('comment-id');
        const report_id = this.getAttribute('report_id');
        const created_at = this.getAttribute('created_at');
        const timeago = this.getAttribute('timeago');
        const name = this.getAttribute('name');
        const photo_path_thumbnail = this.getAttribute('photo_path_thumbnail');

        const user_name = this.getAttribute('user_name');
        const comment = this.getAttribute('comment');
        this.innerHTML = html`
            <div id="contribution_${report_id}" style="display: flex; margin: 0.25rem 0;">
                <div style="align-content: start; width: 100%;">
                    <div style="display: flex; flex-direction: row; justify-content: space-between;">
                        <div style="display: flex;">
                            <div style="font-size: small;">${created_at}</div>
                            <div style="font-size: small; margin-left: 0.25rem;">(${user_name})</div>
                        </div>
                        <div style="font-size: small; margin-right: 0.5rem;">${timeago}</div>
                    </div>
                    <div style="font-weight: bold; font-size: small;">${name}</div>
                    <div style="display: flex; flex-direction: row; align-items: center; justify-content: space-between;">
                        <div style="display: flex; flex-direction: row; align-items: center;">
                            ${photo_path_thumbnail && photo_path_thumbnail !== "null" ? `<img style="width: 2rem; height: 2rem; margin: 0 0.5rem; border-radius: 0.125rem;" src="${photo_path_thumbnail}" alt="photo">` : ''}
                            <div style="font-size: small; color: #4b5563;">${comment}</div>
                        </div>
                        <md-icon-button id="reply_${report_id}_${comment_id || ''}" style="margin-left: 0.5rem; --md-icon-button-icon-size: 1.25rem;">
                            <span class="material-icons" style="font-size: 1.25rem;">reply</span>
                        </md-icon-button>
                    </div>
                </div>
            </div>
            <hr style="border: none; border-bottom: 1px solid #e5e7eb; margin: 0.5rem 0;">
        `;

        // Le bouton Reply a pour ID "reply_{report_id}_{comment_id}" ou "reply_{report_id}" si pas de comment_id
        const replyBtn = this.querySelector(`[id^="reply_${report_id}"]`);
        replyBtn?.addEventListener('click', async (e) => {
            e.stopPropagation();
            const parts = replyBtn.id.replace('reply_', '').split('_');
            const report_id = parts[0];
            const comment_id = parts[1] || null;
            const parentCommentId = comment_id ? parseInt(comment_id) : null;
            await showReplyForm(report_id, parentCommentId);
        });
    }
}

customElements.define('infopanel-contribution', InfopanelContribution);
