import { getViMain } from '/custom-elements/vi-context.js';
let html = String.raw;
class ViMenu extends HTMLElement {
  constructor() {
    super();

    this.innerHTML = html`
            <style>
                vi-menu {
                    position: absolute;
                    top: 0.25em;
                    left: 0.25em;
                    z-index: 20;
                }

                vi-menu md-menu-item img {
                    width: 1.5em;
                    height: 1.5em;
                    margin-right: 0.5em;
                }

                vi-menu md-menu-item {
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
                    <div style="display: flex;">
                        <img src="/pub/install/apple.png">
                        <div style="font-size: 1.125rem;">
                            Installer sur IOS
                        </div>
                    </div>
                </md-menu-item>
                <md-menu-item id="android" style="display: none;">
                    <div style="display: flex;">
                        <img src="/pub/install/android.png">
                        <div style="font-size: 1.125rem;">
                            Installer sur Android
                        </div>
                    </div>
                </md-menu-item>
                <md-menu-item>
                    <a href="https://masto.bike/@veloinfo" target="_blank" style="display: flex;">
                        <div style="display: flex;">
                            <img src="/pub/logo-big.png">
                            <div style="font-size: 1.125rem;">
                                Infos et nouvelles
                            </div>
                        </div>
                    </a>
                </md-menu-item>
                <md-menu-item>
                    <a href="/fediverse" style="display: flex; align-items: center;">
                        <span class="material-icons" style="font-size: 1.5rem; margin-right: 0.5em;">public</span>
                        <div style="font-size: 1.125rem;">
                            Commentaires sur le Fédivers
                        </div>
                    </a>
                </md-menu-item>
                <md-menu-item id="fediverse-login">
                    <div style="display: flex; align-items: center;">
                        <span class="material-icons" style="font-size: 1.5rem; margin-right: 0.5em;">account_circle</span>
                        <div id="fediverse-label" style="font-size: 1.125rem;">
                            Se connecter avec le Fédivers
                        </div>
                    </div>
                </md-menu-item>
                <md-menu-item id="osm-edit">
                    <div style="display: flex;">
                        <img src="/pub/osm.svg">
                        <div style="font-size: 1.125rem;">
                            Editer sur OpenStreetMap
                        </div>
                    </div>
                </md-menu-item>
                <md-menu-item id="route-verte">
                    <div style="display: flex; align-items: center;">
                        <span class="material-icons" id="rv-icon" style="font-size: 1.5rem; margin-right: 0.5em;">eco</span>
                        <div style="font-size: 1.125rem; flex: 1;">
                            Route verte
                        </div>
                        <span class="material-icons" id="rv-check" style="font-size: 1.25rem; color: #16a34a; display: none;">check</span>
                    </div>
                </md-menu-item>
                    <a href="https://openstreetmap.org/copyright" target="_blank">
                        <div style="font-size: 0.75em; color: gray; margin-top: 0.5em;">
                            Les données cartographiques sont fournies par OpenStreetMap
                        </div>
                    </a>
            </md-menu>

            <md-dialog id="fediverse-dialog" style="max-width: 32rem;">
                <div slot="headline">Mon compte Fédivers</div>
                <div slot="content" id="fediverse-content">
                    <img src="/pub/bars.svg" alt="Chargement" style="display: block; margin: 1rem auto;">
                </div>
                <div slot="actions">
                    <md-button id="fediverse-close">Fermer</md-button>
                </div>
            </md-dialog>

            <vi-install-android id="android-install"></vi-install-android>
            <vi-install-ios id="ios-install"></vi-install-ios>
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
        this.querySelector('vi-install-ios').setAttribute('open', 'true');
      });
    } else if (os === 'android') {
      this.querySelector('#android').style.display = 'block';
      this.querySelector('#android').addEventListener('click', () => {
        this.querySelector('vi-install-android').setAttribute('open', 'true');
      });
    }

    this.querySelector('#osm-edit').addEventListener('click', () => this.clickOSMEdit());

    // Route verte : bascule le mode rv=1 (recharge la page)
    const rvCheck = this.querySelector('#rv-check');
    const params = new URLSearchParams(window.location.search);
    if (params.get('rv') === '1') {
      rvCheck.style.display = 'inline';
    }
    this.querySelector('#route-verte').addEventListener('click', () => {
      const url = new URL(window.location);
      if (url.searchParams.get('rv') === '1') {
        url.searchParams.delete('rv');
      } else {
        url.searchParams.set('rv', '1');
      }
      window.location.assign(url);
    });

    // Connexion au compte Fédivers de l'utilisateur
    this.fediverseDialog = this.querySelector('#fediverse-dialog');
    this.querySelector('#fediverse-close').addEventListener('click', () => {
      this.fediverseDialog.close();
    });
    this.querySelector('#fediverse-login').addEventListener('click', async () => {
      await this.refreshFediverse();
      this.fediverseDialog.show();
    });

    // État initial dans le menu (sans bloquer le chargement de la carte).
    this.refreshFediverse();

    // Retour du parcours OAuth : ouvre le dialogue et nettoie l'URL.
    const fediverseStatus = params.get('fediverse');
    if (fediverseStatus) {
      this.refreshFediverse().then(() => {
        if (fediverseStatus === 'error') {
          this.fediverseDialog.querySelector('#fediverse-content').insertAdjacentHTML(
            'afterbegin',
            '<p style="color: #b91c1c; margin: 0 0 1rem;">La connexion a échoué. Vérifiez le domaine de votre instance et réessayez.</p>'
          );
        }
        this.fediverseDialog.show();
      });
      params.delete('fediverse');
      const query = params.toString();
      window.history.replaceState({}, '', window.location.pathname + (query ? `?${query}` : ''));
    }
  }

  escapeHtml(value) {
    return String(value ?? '').replace(/[&<>"']/g, (character) => ({
      '&': '&amp;',
      '<': '&lt;',
      '>': '&gt;',
      '"': '&quot;',
      "'": '&#39;'
    })[character]);
  }

  safeUrl(value) {
    try {
      const url = new URL(value);
      if (url.protocol === 'http:' || url.protocol === 'https:') {
        return url.href;
      }
    } catch (error) {
      // URL invalide : on retombe sur la carte.
    }
    return '/';
  }

  async refreshFediverse() {
    const label = this.querySelector('#fediverse-label');
    const content = this.querySelector('#fediverse-content');
    if (!label || !content) {
      return;
    }

    let data = { connected: false };
    try {
      const response = await fetch('/fediverse/me', { credentials: 'same-origin' });
      if (response.ok) {
        data = await response.json();
      }
    } catch (error) {
      // Le menu reste utilisable même si l'état est indisponible.
    }

    if (data.connected) {
      const account = data.account || {};
      const acct = this.escapeHtml(account.acct || '');
      const profileUrl = this.safeUrl(account.profile_url || account.instance || '/');
      label.textContent = `Fédivers : @${account.acct || ''}`;
      content.innerHTML = `
        <p>Vos nouveaux signalements sont publiés automatiquement sur
          <a href="${profileUrl}" target="_blank" rel="noopener noreferrer">@${acct}</a>.</p>
        <p style="color: #6b7280; font-size: small;">
          Les commentaires et réponses restent fédérés par le compte Vélo info.
        </p>
        <md-outlined-button id="fediverse-disconnect">Se déconnecter</md-outlined-button>
      `;
      content.querySelector('#fediverse-disconnect').addEventListener('click', async () => {
        try {
          await fetch('/fediverse/disconnect', {
            method: 'POST',
            credentials: 'same-origin'
          });
        } finally {
          window.location.assign('/?fediverse=disconnected');
        }
      });
    } else {
      label.textContent = 'Se connecter avec le Fédivers';
      content.innerHTML = `
        <p>Connectez votre compte Mastodon, GoToSocial, Pleroma ou Akkoma.
          Vos signalements seront publiés sur votre compte.</p>
        <form method="post" action="/fediverse/connect"
              style="display: flex; flex-direction: column; gap: 0.75rem; margin: 1rem 0;">
          <md-outlined-text-field name="instance" label="Instance"
              placeholder="mastodon.social, masto.bike…" required></md-outlined-text-field>
          <md-filled-button type="submit">Continuer vers mon instance</md-filled-button>
        </form>
        <p style="color: #6b7280; font-size: small;">
          Vélo info ne reçoit jamais votre mot de passe : vous autorisez
          l'application directement sur votre instance.
        </p>
      `;
    }
  }

  clickOSMEdit() {
    const map = getViMain().map;
    let zoom = map && map.getZoom();
    let lat = map && map.getCenter().lat;
    let lng = map && map.getCenter().lng;
    window.open(`https://www.openstreetmap.org/#map=${zoom}/${lat}/${lng}&layers=C`, '_blank');
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

customElements.define('vi-menu', ViMenu);
