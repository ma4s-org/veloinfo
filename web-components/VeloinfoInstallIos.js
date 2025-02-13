class veloinfoInstallIos extends HTMLElement {
    static observedAttributes = ['open'];

    constructor() {
        super();
        this.innerHTML = `
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
                <div slot="headline">Installer Véloinfo sur iOS</div>
                <div slot="content">
                    <p>1. Touchez le bouton partager</p>
                    <img src="/pub/install/ios-share.jpg" alt="Share button">
                    <p>2. Tap "Ajouter à l'écran d'accueil"</p>
                    <p>3. Tap "Ajouter"</p>
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

customElements.define('veloinfo-install-ios', veloinfoInstallIos);