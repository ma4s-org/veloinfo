import ViPhotoScroll from "./vi-photo-scroll.js";

class SegmentPanel extends HTMLElement {
    constructor(data) {
        super();
        this.data = data;
    }

    set data(data) {
        let photo_ids = data.photo_ids;
        let photos = photo_ids ? photo_ids.map(id => /*html*/`
            <img  style="height: 6rem; border-radius: 0.375rem; padding: 0.5rem; cursor: pointer;" src="/images/${id}_thumbnail.jpeg" alt="photo"
                hx-get="/photo_scroll/${id}/${this.getAttribute('way_ids')}" hx-target="#photo_scroll">
        `).join('') : '';

        let inner = '';
        if (data.edit) {
            inner = /*html*/`
                <form>
                    <score-selector score="${data.score_selector.score}" category="${data.score_selector.category}"></score-selector>
                    <input type="hidden" name="way_ids" value="${data.way_ids}">
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
            inner = /*html*/`
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
                            <md-filled-button id="route_md"><img slot="icon"
                                    src="/pub/directions.png" style="width: 1rem; height: 1rem; margin-right: 0.25rem;">itinéraire</md-filled-button>
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

        let innerHTML = /*html*/`
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
                        ${data.history.map(contribution => /*html*/`
                            <div style="padding: 0.5rem; border-bottom: 1px solid #e5e7eb;">
                                <infopanel-contribution
                                    created_at="${contribution.created_at}"
                                    timeago="${contribution.timeago}"
                                    name="${contribution.name}"
                                    photo_path_thumbnail="${contribution.photo_path_thumbnail}"
                                    score_id="${contribution.score_id}"
                                    score="${contribution.score_circle.score}"
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
            let response = await fetch('/segment_panel', {
                method: 'POST',
                body: new FormData(this.querySelector('form'))
            });
            event.preventDefault();
            let data = await response.json();
            let segment_panel = new SegmentPanel(data);
            document.querySelector('#info').innerHTML = "";
            document.querySelector('#info').appendChild(segment_panel);
        });
        this.querySelector('#cancel')?.addEventListener('click', async (event) => {
            let data = (await fetch(`/info_panel/down`)).json();
            document.querySelector('#info').innerHTML = "<vi-info></vi-info>";
            document.querySelector('vi-info').data = data;
            document.querySelector('vi-main').clear();
            event.preventDefault();
        });

        if (!data.edit) {
            this.querySelector('#route_md').onclick = () => {
                document.querySelector('vi-main').route();
            };
            this.querySelector('#edit_md').onclick = async () => {
                let r = await fetch(`/segment_panel/edit/ways/${data.way_ids}`);
                let dataJson = await r.json();
                let segment_panel = new SegmentPanel(dataJson);
                document.querySelector('#info').innerHTML = '';
                document.querySelector('#info').appendChild(segment_panel);
            };
        }

        let map = document.querySelector('vi-main').map;
        var geom = JSON.parse(data.geom_json);

        if (data.fit_bounds) {
            console.log("fit");
            let flattened = geom.reduce((acc, val) => acc.concat(val), []);
            map.fitBounds(document.querySelector('vi-main').fitBounds(flattened), { padding: window.innerHeight * .12 });
        }
        if (!window.start_marker) {
            window.start_marker = new window.maplibregl.Marker({ color: "#00f" }).setLngLat(geom[0][0]).addTo(map);
        }
        if (!data.edit) {
            if (map.getLayer("selected")) {
                map.getSource("selected").setData({
                    "type": "Feature",
                    "properties": {},
                    "geometry": {
                        "type": "MultiLineString",
                        "coordinates": geom
                    }
                });
            } else {
                map.addSource("selected", {
                    "type": "geojson",
                    "data": {
                        "type": "Feature",
                        "properties": {},
                        "geometry": {
                            "type": "MultiLineString",
                            "coordinates": geom
                        }
                    }
                })
                map.addLayer({
                    "id": "selected",
                    "type": "line",
                    "source": "selected",
                    "paint": {
                        "line-width": 8,
                        "line-color": "hsl(205, 100%, 50%)",
                        "line-blur": 0,
                        "line-opacity": 0.50
                    }
                },
                    "Road labels")
            }
            map.getSource("bike_path").setUrl(`${window.location.origin}/bike_path?t=${Date.now()}`);
        }
    }
}
export default SegmentPanel;
customElements.define('vi-segment-panel', SegmentPanel);

class ScoreCircle extends HTMLElement {
    constructor() {
        super();
    }

    connectedCallback() {
        const score = parseFloat(this.getAttribute('score'));
        if (score === 1.0) {
            this.innerHTML = /*html*/` 
                <div style="border-radius: 9999px; background-color: #064e3b; height: 2rem; width: 2rem; margin: 0.25rem;"></div>
                `;
        } else if (score >= 0.60) {
            this.innerHTML = /*html*/` 
                <div style="border-radius: 9999px; background-color: #fbbf24; height: 2rem; width: 2rem; margin: 0.25rem;"></div>
                `;
        } else if (score >= 0.30) {
            this.innerHTML = /*html*/` 
                <div style="border-radius: 9999px; background-color: #ea580c; height: 2rem; width: 2rem; margin: 0.25rem;"></div>
                `;
        } else if (score === 0.0) {
            this.innerHTML = /*html*/` 
                <div style="border-radius: 9999px; background-color: #b91c1c; height: 2rem; width: 2rem; margin: 0.25rem;"></div>
                `;
        } else {
            this.innerHTML = /*html*/` 
                <div style="border-radius: 9999px; background-color: #9ca3af; height: 2rem; width: 2rem; margin: 0.25rem;"></div>
                `;
        }
    }
}

customElements.define('score-circle', ScoreCircle);

class InfopanelContribution extends HTMLElement {
    constructor() {
        super();
    }

    connectedCallback() {
        const score = this.getAttribute('score');
        const score_id = this.getAttribute('score_id');
        const created_at = this.getAttribute('created_at');
        const timeago = this.getAttribute('timeago');
        const name = this.getAttribute('name');
        const photo_path_thumbnail = this.getAttribute('photo_path_thumbnail');

        const user_name = this.getAttribute('user_name');
        const comment = this.getAttribute('comment');
        this.innerHTML = /*html*/`
            <div id="contribution_${score_id}" style="cursor: pointer; display: flex; margin: 0.25rem 0;">
                <score-circle score="${score}"></score-circle>
                <div  style="align-content: start; width: 100%;">
                    <div style="display: flex; flex-direction: row; justify-content: space-between;">
                        <div style="display: flex;">
                                <div style="font-size: small;"> ${created_at} </div>
                                <div style="font-size: small; margin-left: 0.25rem;"> ( ${user_name} ) </div>
                        </div>
                        <div style="font-size: small; margin-right: 0.5rem;"> ${timeago} </div>
                    </div>
                    <div style="font-weight: bold; font-size: small;">${name}</div>
                    <div style="display: flex; flex-direction: row; align-items: center;">
                        ${photo_path_thumbnail && photo_path_thumbnail !== "null" ? `<img style="width: 2rem; height: 2rem; margin: 0 0.5rem; border-radius: 0.125rem;" src="${photo_path_thumbnail}" alt="photo">` : ''}    
                        <div  style="font-size: small; color: #4b5563;">${comment}</div>
                    </div>
                </div>
            </div>
            <hr>        
        `;

        this.querySelector(`#contribution_${score_id}`)?.addEventListener('click', async () => {
            let r = await fetch(`/segment_panel/id/${score_id}`);
            let data = await r.json();
            let segment_panel = new SegmentPanel(data);
            document.querySelector('#info').innerHTML = '';
            document.querySelector('#info').appendChild(segment_panel);
        })

    }
}

customElements.define('infopanel-contribution', InfopanelContribution);

class ScoreSelector extends HTMLElement {
    constructor() {
        super();
    }
    connectedCallback() {
        let category = this.getAttribute('category');
        let score = this.getAttribute('score');

        let categoryDiv = '';
        if (category == 'Good') {
            categoryDiv = `<div style="display: flex; flex-direction: row; cursor: pointer;">
                <div style="background-color: rgb(20,83,45); 
                            border-color: black; 
                            width: 2rem; 
                            height: 2rem; 
                            border-radius: 9999px; 
                            border-width: 4px;"></div>
                <div style="margin: 0.5rem; align-self: center;">
                    État normal
                </div>
            </div>`;
        } else {
            categoryDiv = `<div id="good" style="display: flex; flex-direction: row; cursor: pointer;">
                <div style="background-color: rgb(20,83,45); 
                            width: 2rem; 
                            height: 2rem; 
                            border-radius: 9999px;"></div>
                <div style="margin: 0.5rem; align-self: center;">
                    État normal
                </div>
            </div>`;
        }
        if (category == 'Problems') {
            categoryDiv += `<div style="display: flex; flex-direction: row; cursor: pointer;">
                <div style="background-color: rgb(234, 179, 8); 
                            border-color: black; 
                            width: 2rem; 
                            height: 2rem; 
                            border-radius: 9999px; 
                            border-width: 4px;"></div>
                <div style="margin: 0.5rem; align-self: center;">
                    Problème mineur (ex: cohabitation avec voitures problématique)
                </div>
            </div>`;
        } else {
            categoryDiv += `<div id="problems" style="display: flex; flex-direction: row; cursor: pointer;">
                <div style="background-color: rgb(234, 179, 8); 
                            width: 2rem; 
                            height: 2rem; 
                            border-radius: 9999px;"></div>
                <div style="margin: 0.5rem; align-self: center;">
                    Problème mineur (ex: cohabitation avec voitures problématique)
                </div>
            </div>`;
        }
        if (category == 'MajorProblems') {
            categoryDiv += `<div style="display: flex; flex-direction: row; cursor: pointer;">
                <div style="background-color: rgb(234,88,12); 
                            border-color: black; 
                            width: 2rem; 
                            height: 2rem; 
                            border-width: 4px;
                            border-radius: 9999px;"></div>
                <div style="margin: 0.5rem; align-self: center;">
                    Piste dangeureuse (ex: piste cyclable en très mauvais état)
                </div>
            </div>`;
        } else {
            categoryDiv += `<div id="major-problems" style="display: flex; flex-direction: row; cursor: pointer;">
                <div style="background-color: rgb(234,88,12);            
                            width: 2rem; 
                            height: 2rem; 
                            border-radius: 9999px;"></div>
                <div style="margin: 0.5rem; align-self: center;">
                    Problème majeur (ex: cohabitation avec voitures problématique)
                </div>
            </div>`;
        }
        if (category == 'Closed') {
            categoryDiv += `<div style="display: flex; flex-direction: row; cursor: pointer;">
                <div style="background-color: rgb(153,27,27);   
                            border-color: black;         
                            width: 2rem; 
                            height: 2rem; 
                            border-radius: 9999px; 
                            border-width: 4px;"></div>
                <div style="margin: 0.5rem; align-self: center;">
                    Fermé (ex: travaux ou neige)
                </div>
            </div>`;
        } else {
            categoryDiv += `<div id="closed" style="display: flex; flex-direction: row; cursor: pointer;">
                <div style="background-color: rgb(153,27,27);            
                            width: 2rem; 
                            height: 2rem; 
                            border-radius: 9999px;"></div>
                <div style="margin: 0.5rem; align-self: center;">
                    Fermé (ex: travaux ou neige)
                </div>
            </div>`;
        }


        this.innerHTML = /*html*/`
        <div id="score_selector"  style="margin: 0.5rem;">
        <div style="font-weight: bold;">Confort :</div>
        ${categoryDiv}
        <input type="hidden" name="score" value="${score}">
        </div>`;

        this.querySelector('#good')?.addEventListener('click', () => {
            this.innerHTML = '<score-selector category="Good" score="1.0"></score-selector>';
        });
        this.querySelector('#problems')?.addEventListener('click', () => {
            this.innerHTML = '<score-selector category="Problems" score="0.6"></score-selector>';
        });
        this.querySelector('#major-problems')?.addEventListener('click', () => {
            this.innerHTML = '<score-selector category="MajorProblems" score="0.3"></score-selector>';
        });
        this.querySelector('#closed')?.addEventListener('click', () => {
            this.innerHTML = '<score-selector category="Closed" score="0.0"></score-selector>';
        });
    }
}

customElements.define('score-selector', ScoreSelector);     