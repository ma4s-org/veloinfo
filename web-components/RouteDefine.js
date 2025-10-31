class RouteDefine extends HTMLElement {
    constructor() {
        super();
        var destination = window.start_marker.getLngLat();
    }

    connectedCallback() {
        this.render();
    }

    render() {
        let innerHTML = /*html*/ `
            <style>
                route-define search-input {
                    justify-content: center;
                    display: flex;
                }

                route-define search-input #top {
                    display: flex;
                    justify-content: center;
                    z-index: 10;
                    position: static;
                    margin: 0.5em;
                    top: 0.88rem;
                    flex-direction: column;
                }

                route-define search-input #query {
                    outline: solid;
                    text-align: center;
                    border-radius: 0.5rem;
                    width: 13.25rem;
                    height: 2.25rem;
                } 
            </style>
            <div class="absolute w-full overflow-auto md:w-[500px] bg-white z-20 bottom-0 rounded-lg">
                Vous partez de votre position actuelle ou vous pouvez entrer le point de départ : <br>
                <search-input></search-input>
                <div class="flex justify-center">
                    <md-filled-button hx-on:click="defineRoute()" hx-target="#info">définir l'itinéraire</md-filled-button>
                </div>
            </div>
        `;
        this.innerHTML = innerHTML;
    }
}

customElements.define('route-define', RouteDefine);