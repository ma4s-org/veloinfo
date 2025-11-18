class SegmentPanel extends HTMLElement {
    constructor() {
        super();
    }

    set data(data) {
        let photo_ids = data.photo_ids;
        let photos = photo_ids ? photo_ids.map(id => /*html*/`
            <img class="h-24 rounded-md p-2 cursor-pointer" src="/images/${id}_thumbnail.jpeg" alt="photo"
                hx-get="/photo_scroll/${id}/${this.getAttribute('way_ids')}" hx-target="#photo_scroll">
        `).join('') : '';

        let inner = '';
        if (data.edit) {
            inner = /*html*/`
                <form>
                    <score-selector score="${data.score_selector.score}" category="${data.score_selector.category}"></score-selector>
                    <input type="hidden" name="way_ids" value="${data.way_ids}">
                    <input type="text" name="user_name" class="border-2" placeholder="Nom" value="${data.user_name}">
                    <textarea rows="4" cols="50" name="comment" class="border-2" placeholder="Commentaire"></textarea>
                    <div class="uppercase m-2">
                        <label for="photo">Choisissez une photo :</label>
                        <input type="file" id="photo" name="photo">
                    </div>
                    <div class="flex justify-center">
                        <md-filled-button id="save" type="button">Enregistrer</md-filled-button>
                        <md-filled-button id="cancel" type="button">annuler</md-filled-button>  
                    </div>
                </form>
            `;
        } else {
            inner = /*html*/`
                <div>
                    <div class="flex mb-2 mt-1">
                        <div>
                            <div class="text-sm font-bold">${data.segment_name}</div>
                            <div class="text-sm text-gray-600">${data.comment}</div>
                        </div>
                    </div>
                    <div class="">
                        <div class="flex flex-row justify-center">
                            Choisissez un second point pour aggrandir la sélection ou
                        </div>
                        <div class="flex flex-row justify-center">
                            <md-filled-button id="route_md"><img slot="icon"
                                    src="/pub/directions.png" class="w-4 h-4 mr-1">itinéraire</md-filled-button>
                            <md-filled-button id="edit_md"><img slot="icon"
                                    src="/pub/edit.png" class="w-4 h-4 mr-1">
                                Modifier</md-filled-button>
                        </div>
                        <div class="flex flex-row justify-center p-2">
                            <md-filled-button
                                id="cancel">annuler</md-filled-button>
                        </div>
                    </div>
                </div>
            `;
        }

        let innerHTML = /*html*/`
            <div id="segment_panel"
                class="absolute w-full max-h-[50%] overflow-auto md:w-[500px] bg-white z-20 bottom-0 rounded-lg">
                <img id="spinner" class="htmx-indicator absolute z-30 bottom-8 mx-auto inset-x-0 top-1" src="/pub/bars.svg" />
                <div class="p-2 m-1">
                    ${inner}
                    <div class="flex flex-row overflow-auto">
                        ${data.photo_ids.map(photo_id => /*html*/`
                            <img style="height: 6rem; cursor: pointer;"
                                    src="/images/${photo_id}_thumbnail.jpeg" alt="photo"
                                    hx-get="/photo_scroll/${photo_id}/${data.way_ids}" hx-target="#photo_scroll">
                        `).join('')}
                    </div>
                    <div id="photo_scroll"></div>
                    <div class="uppercase m-2">historique</div>
                    <div class="overflow-auto max-h-48 md:h-[500px]">
                        <hr>
                        ${data.history.map(contribution => /*html*/`
                            <div class="p-2 border-b border-gray-200">
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
        this.querySelector('#save')?.addEventListener('click', async (event) => {
            that.querySelector('#save').disabled = true;
            let response = await fetch('/segment_panel', {
                method: 'POST',
                body: new FormData(this.querySelector('form'))
            });
            event.preventDefault();
            let data = await response.json();
            document.querySelector('#info').innerHTML = "<segment-panel></segment-panel>";
            document.querySelector('segment-panel').data = data;
        });
        this.querySelector('#cancel')?.addEventListener('click', async (event) => {
            let innerHTML = await fetch(`/info_panel/down`);
            document.querySelector('#info').innerHTML = innerHTML;
            document.querySelector('veloinfo-map').clear();
            event.preventDefault();
        });

        if (!data.edit) {
            this.querySelector('#route_md').onclick = () => {
                document.querySelector('veloinfo-map').route();
            };
            this.querySelector('#edit_md').onclick = async () => {
                let r = await fetch(`/segment_panel/edit/ways/${data.way_ids}`);
                let dataJson = await r.json();
                document.querySelector('#info').innerHTML = '<segment-panel></segment-panel>';
                document.querySelector('segment-panel').data = dataJson;
            };
        }

        let map = document.querySelector('veloinfo-map').map;
        var geom = JSON.parse(data.geom_json);

        if (this.getAttribute('fit_bounds') == 'true') {
            map.fitBounds(document.querySelector('veloinfo-map').fitBounds(geom[0]), { padding: window.innerHeight * .12 });
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
            map.getSource("veloinfo").setUrl(`${window.location.origin}/bike_path`);

        }
    }
}

customElements.define('segment-panel', SegmentPanel);

class ScoreCircle extends HTMLElement {
    constructor() {
        super();
    }

    connectedCallback() {
        const score = parseFloat(this.getAttribute('score'));
        if (score === 1.0) {
            this.innerHTML = /*html*/` 
                <div class="rounded-full bg-green-900 h-8 w-8 m-1"></div>
                `;
        } else if (score >= 0.60) {
            this.innerHTML = /*html*/` 
                <div class="rounded-full bg-yellow-400 h-8 w-8 m-1"></div>
                `;
        } else if (score >= 0.30) {
            this.innerHTML = /*html*/` 
                <div class="rounded-full bg-orange-600 h-8 w-8 m-1"></div>
                `;
        } else if (score === 0.0) {
            this.innerHTML = /*html*/` 
                <div class="rounded-full bg-red-800 h-8 w-8 m-1"></div>
                `;
        } else {
            this.innerHTML = /*html*/` 
                <div class="rounded-full bg-gray-400 h-8 w-8 m-1"></div>
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
            <div class="flex cursor-pointer mb-2 mt-1" hx-get="/segment_panel/id/${score_id}" hx-target="#info">
                <score-circle score="${score}"></score-circle>
                <div class="content-start w-full">
                    <div class="flex flex-row justify-between">
                        <div class="flex">
                                <div class="text-xs"> ${created_at} </div>
                                <div class="text-xs ml-1"> ( ${user_name} ) </div>
                        </div>
                        <div class="text-xs mr-2"> ${timeago} </div>
                    </div>
                    <div class="font-bold text-sm">${name}</div>
                    <div class="flex flex-row">
                        ${photo_path_thumbnail ? `<img class="w-8 h-8 mx-2 rounded-sm" src="${photo_path_thumbnail}" alt="photo">` : ''}    
                        <div class="text-sm text-gray-600">${comment}</div>
                    </div>
                </div>
            </div>
            <hr>        
        `;
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
            categoryDiv = `<div class="flex flex-row cursor-pointer">
                <div style="background-color: rgb(20,83,45); 
                            border-color: black; 
                            width: 2rem; 
                            height: 2rem; 
                            border-radius: 9999px; 
                            border-width: 4px;"></div>
                <div class="m-2 align-middle">
                    État normal
                </div>
            </div>`;
        } else {
            categoryDiv = `<div id="good" class="flex flex-row cursor-pointer">
                <div style="background-color: rgb(20,83,45); 
                            width: 2rem; 
                            height: 2rem; 
                            border-radius: 9999px;"></div>
                <div class="m-2 align-middle">
                    État normal
                </div>
            </div>`;
        }
        if (category == 'Problems') {
            categoryDiv += `<div class="flex flex-row cursor-pointer">
                <div style="background-color: rgb(234, 179, 8); 
                            border-color: black; 
                            width: 2rem; 
                            height: 2rem; 
                            border-radius: 9999px; 
                            border-width: 4px;"></div>
                <div class="m-2 align-middle">
                    Problème mineur (ex: cohabitation avec voitures problématique)
                </div>
            </div>`;
        } else {
            categoryDiv += `<div id="problems" class="flex flex-row cursor-pointer">
                <div style="background-color: rgb(234, 179, 8); 
                            width: 2rem; 
                            height: 2rem; 
                            border-radius: 9999px;"></div>
                <div class="m-2 align-middle">
                    Problème mineur (ex: cohabitation avec voitures problématique)
                </div>
            </div>`;
        }
        if (category == 'MajorProblems') {
            categoryDiv += `<div class="flex flex-row cursor-pointer">
                <div style="background-color: rgb(234,88,12); 
                            border-color: black; 
                            width: 2rem; 
                            height: 2rem; 
                            border-width: 4px;
                            border-radius: 9999px;"></div>
                <div class="m-2 align-middle">
                    Piste dangeureuse (ex: piste cyclable en très mauvais état)
                </div>
            </div>`;
        } else {
            categoryDiv += `<div id="major-problems" class="flex flex-row cursor-pointer">
                <div style="background-color: rgb(234,88,12);            
                            width: 2rem; 
                            height: 2rem; 
                            border-radius: 9999px;"></div>
                <div class="m-2 align-middle">
                    Problème majeur (ex: cohabitation avec voitures problématique)
                </div>
            </div>`;
        }
        if (category == 'Closed') {
            categoryDiv += `<div class="flex flex-row cursor-pointer">
                <div style="background-color: rgb(153,27,27);   
                            border-color: black;         
                            width: 2rem; 
                            height: 2rem; 
                            border-radius: 9999px; 
                            border-width: 4px;"></div>
                <div class="m-2 align-middle">
                    Fermé (ex: travaux ou neige)
                </div>
            </div>`;
        } else {
            categoryDiv += `<div id="closed" class="flex flex-row cursor-pointer">
                <div style="background-color: rgb(153,27,27);            
                            width: 2rem; 
                            height: 2rem; 
                            border-radius: 9999px;"></div>
                <div class="m-2 align-middle">
                    Fermé (ex: travaux ou neige)
                </div>
            </div>`;
        }


        this.innerHTML = /*html*/`
        <div id="score_selector" class="m-2">
        <div class="font-bold">Confort :</div>
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