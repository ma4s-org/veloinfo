class VeloinfoMenu extends HTMLElement {
    constructor() {
        super();

        this.innerHTML = `
            <style>
                veloinfo-menu {
                    position: absolute;
                    top: 0.25em;
                    left: 0.25em;
                    z-index: 20;
                }

                veloinfo-menu md-menu-item img {
                    width: 1.5em;
                    height: 1.5em;
                    margin-right: 0.5em;
                }

                veloinfo-menu md-menu-item {
                    width: 25em;
                }
            </style>
            <md-icon-button
                id="vi-menu"
                aria-label="Open menu"
                aria-label-selected="Close menu"
                toggle>
                <md-icon class="material-icons">menu</md-icon>
                <md-icon class="material-icons" slot="selected">close</md-icon>
            </md-icon-button>
        
            <md-menu anchor="vi-menu">
                <md-menu-item id="ios" style="display: none;">
                    <div class="flex" hx-get="/menu/closed" hx-target="#menu" hx-swap="outerHTML">
                        <img src="/pub/install/apple.png">
                        <div class="text-lg">
                            Installer sur IOS
                        </div>
                    </div>
                </md-menu-item>
                <md-menu-item id="android" style="display: none;">
                    <div class="flex" hx-get="/menu/closed" hx-target="#menu" hx-swap="outerHTML">
                        <img src="/pub/install/android.png">
                        <div class="text-lg">
                            Installer sur Android
                        </div>
                    </div>
                </md-menu-item>
                <md-menu-item>
                    <a href="https://masto.bike/@veloinfo" target="_blank" class="flex">
                        <div class="flex">
                            <img src="/pub/logo-big.png">
                            <div class="text-lg">
                                Infos et nouvelles
                            </div>
                        </div>
                    </a>
                </md-menu-item>
                <md-menu-item id="osm-edit">
                    <div class="flex">
                        <img src="/pub/osm.svg">
                        <div class="text-lg">
                            Editer sur OpenStreetMap
                        </div>
                    </div>
                </md-menu-item>
            </md-menu>

            <veloinfo-install-android id="android-install"></veloinfo-install-android>
            <veloinfo-install-ios id="ios-install"></veloinfo-install-ios>
            `;
    }

    connectedCallback() {
        this.querySelector('#vi-menu').addEventListener('click', () => {
            this.querySelector('md-menu').open = !this.querySelector('md-menu').open;
        });

        // When closing the menu, deselect the button
        const menu = this.querySelector('md-menu');
        const observer = new MutationObserver((mutationsList) => {
            for (const mutation of mutationsList) {
                if (mutation.type === 'attributes' && mutation.attributeName === 'open') {
                    if (!menu.open) {
                        this.querySelector('md-icon-button').selected = false;
                    }
                }
            }
        });
        observer.observe(menu, { attributes: true });

        // Add install links
        const os = this.detectMobileOS();

        if (os === 'ios') {
            this.querySelector('#ios').style.display = 'block';
            this.querySelector('#ios').addEventListener('click', () => {
                this.querySelector('veloinfo-install-ios').setAttribute('open', 'true');
            });
        } else if (os === 'android') {
            this.querySelector('#android').style.display = 'block';
            this.querySelector('#android').addEventListener('click', () => {
                this.querySelector('veloinfo-install-android').setAttribute('open', 'true');
            });
        }

        this.querySelector('#osm-edit').addEventListener('click', () => this.clickOSMEdit());
    }

    clickOSMEdit() {
        const map = document.querySelector('map-div').map;
        let zoom = map && map.getZoom();
        let lat = map && map.getCenter().lat;
        let lng = map && map.getCenter().lng;
        window.open(`https://www.openstreetmap.org/#map=${zoom}/${lat}/${lng}&layers=Y`, '_blank');
    }

    detectMobileOS() {
        const userAgent = navigator.userAgent || navigator.vendor || window.opera;

        // iOS detection
        if (/iPad|iPhone|iPod/.test(userAgent) && !window.MSStream) {
            return 'ios';
        }

        // Android detection
        if (/android/i.test(userAgent)) {
            return 'android';
        }

        return 'unknown';
    }
}

customElements.define('veloinfo-menu', VeloinfoMenu);