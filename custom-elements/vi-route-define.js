import { getViMain } from '/custom-elements/vi-context.js';

class RouteDefine extends HTMLElement {
    constructor() {
        super();
        const viMain = getViMain();
        var destination = viMain.start_marker.getLngLat();
    }

    connectedCallback() {
        this.render();
    }

    render() {
        let innerHTML = /*html*/ `
            <style>
                vi-route-define vi-search-input {
                    justify-content: center;
                    display: flex;
                }
                
                vi-route-define vi-search-input #top {
                    display: flex;
                    justify-content: center;
                    z-index: 10;
                    position: static;
                    margin: 0.5em;
                    top: 0.88rem;
                    flex-direction: column;
                }

                vi-route-define vi-search-input #query {
                    outline: solid;
                    text-align: center;
                    border-radius: 0.5rem;
                    width: 13.25rem;
                    height: 2.25rem;
                } 
            </style>
            <div class="vi-panel">
                Vous partez de votre position actuelle ou vous pouvez entrer le point de départ : <br>
                <vi-search-input></vi-search-input>
                <div style="display: flex; justify-content: center;">
                    <md-filled-button hx-on:click="defineRoute()" hx-target="#info">définir l'itinéraire</md-filled-button>
                </div>
            </div>
        `;
        this.innerHTML = innerHTML;
    }
}

customElements.define('vi-route-define', RouteDefine);