class veloinfoInstallAndroid extends HTMLElement {
    static observedAttributes = ['open'];

    constructor() {
        super();
        this.innerHTML = /*html*/ `
            <style>
                .install-dialog {
                    max-width: 400px;
                }
                .install-dialog img {
                    width: 12em;
                    margin: 1em;
                }
            </style>
            <md-dialog class="install-dialog" id="ios-install">
                <div slot="headline">Installer Véloinfo sur Android</div>
                <div slot="content">
                    <p>1. Sur Chrome touchez le menu</p>
                    <p>2. Touchez "Ajouter à l'écran d'accueil"</p>
                </div>
                <div slot="actions">
                    <md-button id="close-button">Fermer</md-button>
                </div>
            </md-dialog>
        `;

        this.dialog = this.querySelector('#ios-install');
        this.closeButton = this.querySelector('#close-button');

        this.closeButton.addEventListener('click', () => {
            this.dialog.removeAttribute('open');
        });
    }

    attributeChangedCallback(name, oldValue, newValue) {
        if (name === 'open') {
            if (newValue === 'true') {
                this.dialog.setAttribute('open', 'true');
            } else {
                this.dialog.removeAttribute('open');
            }
        }
    }
}

customElements.define('vi-install-android', veloinfoInstallAndroid);