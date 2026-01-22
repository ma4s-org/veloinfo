import PointPanel from "./vi-point-panel.js";
import ViSearchResult from "./vi-search-result.js";

class SearchInput extends HTMLElement {
    query = "";
    abortController = new AbortController();
    constructor() {
        super();
        // upgrade recentTargets to the new format
        // remove in six months that is in 2025-06-01
        let recentTargets = JSON.parse(localStorage.getItem('recentTargets')) || [];
        for (let i = 0; i < recentTargets.length; i++) {
            if (typeof recentTargets[i] === "string") {
                const regex = /Result\((.*?),(.*?),.*?\/div>(.*?)</s;
                let m = recentTargets[i].match(regex);
                if (m.length === 4) {
                    recentTargets[i] = { lng: m[1].trim(), lat: m[2].trim(), name: m[3].trim() };
                }
            }
        }
        localStorage.setItem('recentTargets', JSON.stringify(recentTargets));
    }

    connectedCallback() {
        let innerHTML = /*html*/ `
            <div id="top">
                <form onsubmit="return false;">
                    <input id="query" name="query" type="search"
                        name="query" placeholder="Rechercher" value="${this.query}" autofocus />
                    <input type="hidden" name="lng" value="{{lng}}">
                    <input type="hidden" name="lat" value="{{lat}}">
                </form>
                <div id="search_results" class="bg-white flex flex-col justify-center">
                </div>
            </div>

            <style>
                vi-search-input {
                    justify-content: center;
                    display: flex;
                }

                vi-search-input #top {
                    display: flex;
                    justify-content: center;
                    z-index: 10;
                    position: absolute;
                    top: 0.88rem;
                    flex-direction: column;
                }
                vi-search-input #query {
                    outline: solid;
                    text-align: center;
                    border-radius: 0.5rem;
                    width: 13.25rem;
                    height: 2.25rem;
                } 
            </style>
        `;
        this.innerHTML = innerHTML;
        this.querySelector("#query").addEventListener("keyup", (event) => this.search(event));
        this.querySelector("#query").addEventListener("focusout", (event) => this.clearResult(event));
        this.querySelector("#query").addEventListener("click", (event) => this.search(event));
    }

    async search(event) {
        this.abortController?.abort();
        this.abortController = new AbortController();
        const signal = this.abortController.signal;

        let map = document.querySelector('vi-main').map;
        this.query = this.querySelector("#query").value;
        if (!this.query) {
            this.displayHistory();
            return;
        }
        let lng = map.getCenter().lng;
        let lat = map.getCenter().lat;
        // Si enter on séléctionne le premier résultat
        if (event.key === "Enter") {
            this.querySelector("#search_results div.result").click();
            this.getElementById("search_results").innerHTML = "";
            return;
        }
        let response = await fetch(`/search`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
            body: new URLSearchParams({ query: this.query, lng, lat }).toString(),
            signal
        });
        const searchResults = this.querySelector("#search_results");
        if (searchResults) {
            const results = await response.json();
            let viSearchResult = new ViSearchResult(results.search_results, this.query);
            searchResults.innerHTML = '';
            searchResults.appendChild(viSearchResult);
        }
    }

    displayHistory() {
        this.query = this.querySelector("#query");
        if (query.value === "") { // Vérifier si le champ de recherche est vide
            let recentTargets = JSON.parse(localStorage.getItem('recentTargets')) || [];
            let searchResults = this.querySelector("#search_results");
            if (searchResults) {
                searchResults.innerHTML = ""; // Effacer les résultats de recherche existants
                for (let target of recentTargets) {
                    let searchResult = SearchResult.fromJSON(target);
                    if (searchResult) {
                        searchResults.appendChild(searchResult);
                    }
                }
                htmx.process(searchResults);
            }
        }
    }

    clearResult() {
        setTimeout(() => {
            const searchResults = this.querySelector("#search_results");
            if (searchResults) {
                searchResults.innerHTML = "";
            }
        }, 250);
    }

}

customElements.define('vi-search-input', SearchInput, {});

class SearchResult extends HTMLElement {
    constructor() {
        super();

    }

    connectedCallback() {
        let innerHTML = `
            <div>
                <div class="circle"></div>
                <div class="name">${this.getAttribute("name")}</div>
                <hr>
            </div>

            <style>
                search-result {
                    display: flex;
                    justify-content: center;
                    margin-top: 0.5rem;
                }
                search-result div {
                    margin: 0.25rem;
                    width: 13.25rem;
                    display: flex;
                    align-items: center;
                    cursor: pointer;
                    font-weight: bold;
                }
                search-result .circle {
                    background-color: #983f2f;
                    border-radius: 50%;
                    height: 1rem;
                    width: 1rem;
                    margin-right: 0.5rem;
                }
            </style>
        `;
        this.innerHTML = innerHTML;
        this.querySelector("div").addEventListener("click", () => this.clickSearchResult());
    }

    async clickSearchResult() {
        let map = document.querySelector('vi-main').map;
        let viMain = document.querySelector('vi-main');
        let lng = this.getAttribute("lng");
        let lat = this.getAttribute("lat");
        // Stocker la cible dans le localStorage
        let recentTargets = JSON.parse(localStorage.getItem('recentTargets')) || [];

        let target = this.toJSON();

        // Vérifier si la cible existe déjà dans la liste
        let index = recentTargets.findIndex((element) => element != null && element.lng === target.lng && element.lat === target.lat && element.name === target.name);

        if (index !== -1) {
            // Si oui, supprimer l'ancienne cible de la liste
            recentTargets.splice(index, 1);
        } else if (recentTargets.length >= 15) {
            // Si la liste est pleine et que la cible n'est pas déjà dans la liste, supprimer le plus ancien élément
            recentTargets.pop();
        }

        // Ajouter la nouvelle cible au début de la liste
        recentTargets.unshift(target);
        localStorage.setItem('recentTargets', JSON.stringify(recentTargets));

        // Vérifier si on est en mode changeStart
        const changeStartElement = document.querySelector('vi-change-start');
        if (changeStartElement) {
            // Mode changeStart : mettre à jour le départ et lancer la route
            if (viMain.start_marker) {
                viMain.start_marker.remove();
            }
            viMain.start_marker = new maplibregl.Marker({ color: "#f00" }).setLngLat([lng, lat]).addTo(map);

            // Lancer la route vers la destination existante
            viMain.recalculateRoute("safe");
        } else {
            // Mode normal : changer le point de destination
            if (viMain.start_marker) {
                viMain.start_marker.remove();
            }
            if (viMain.end_marker) {
                viMain.end_marker.remove();
                viMain.end_marker = null;
            }
            // Le point choisi via la recherche est la destination : marqueur bleu
            viMain.start_marker = new maplibregl.Marker({ color: "#00f" }).setLngLat([lng, lat]).addTo(map);
        }

        const searchInput = this.closest('vi-search-input');
        if (searchInput) {
            const searchResults = searchInput.querySelector("#search_results");
            if (searchResults) {
                searchResults.innerHTML = '';
            }
            const queryInput = searchInput.querySelector("#query");
            if (queryInput) {
                queryInput.value = '';
            }
        }

        // En mode normal, afficher le point panel
        if (!changeStartElement) {
            let result = await fetch(`/point_panel_lng_lat/${lng}/${lat}`);
            let json = await result.json();
            let pointPanel = new PointPanel(json.name);
            document.getElementById("info").innerHTML = '';
            document.getElementById("info").appendChild(pointPanel);
        }

        map.flyTo({
            center: [lng, lat],
        });
    }

    toJSON() {
        return {
            lng: this.getAttribute("lng"),
            lat: this.getAttribute("lat"),
            name: this.getAttribute("name"),
        };
    }

    static fromJSON(json) {
        if (!json) {
            return null;
        }
        const element = document.createElement('search-result');
        element.setAttribute("lng", json.lng);
        element.setAttribute("lat", json.lat);
        element.setAttribute("name", json.name);
        return element;
    }

}

customElements.define('search-result', SearchResult, {});
