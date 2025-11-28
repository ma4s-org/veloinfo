export default class ViPhotoScroll extends HTMLElement {
    constructor(data) {
        super();
        this.render(data);
    }

    set data(data) {
        this.render(data);
    }

    render(data) {
        const { photo, previous, next, way_ids } = data || {};

        // store for handlers
        this._previous = previous;
        this._next = next;
        this._way_ids = way_ids;

        this.innerHTML = /*html*/`
        <div popover id="photo_scroll_inner" style="height:100%; max-width:100%; display:flex; align-items:center; justify-content:center; z-index:40;">
            ${previous ? `<button id="previous_button" style="position:absolute; bottom:0; left:0; font-size:1.5rem; background-color: rgba(255,255,255,0.5); padding:2rem; z-index:50;">&lt;</button>` : ''}
            ${next ? `<button id="next_button" style="position:absolute; bottom:0; right:0; font-size:1.5rem; background-color: rgba(255,255,255,0.5); padding:2rem; z-index:50;">&gt;</button>` : ''}
            <img src="/images/${photo}.jpeg" style="object-fit:contain; max-height:100%; max-width:100%; height:100%; width:auto;" alt="photo">
            <button id="close_button" style="position:absolute; top:0; right:0; font-size:1.5rem; background-color: rgba(255,255,255,0.5); padding:2rem; z-index:40;">X</button>
        </div>`;

        // attach listeners after rendering (so they're reattached on every render)
        this._attachListeners();
    }

    _attachListeners() {
        const popover = document.getElementById('photo_scroll_inner');
        if (!popover) return;

        if (popover.parentElement !== document.body) {
            document.body.appendChild(popover);
        }

        const closeBtn = popover.querySelector('#close_button');
        if (closeBtn) {
            closeBtn.onclick = () => {
                popover.remove();
            };
        }

        const prevBtn = popover.querySelector('#previous_button');
        if (prevBtn) {
            prevBtn.onclick = async () => {
                if (!this._previous) return;
                try {
                    const r = await fetch(`/photo_scroll/${this._previous}/${this._way_ids}`);
                    if (!r.ok) return;
                    const data = await r.json();
                    let newPhotoScroll = new ViPhotoScroll(data);
                    document.getElementById('photo_scroll_inner').remove();
                    document.body.appendChild(newPhotoScroll);
                } catch (e) {
                    console.error('Failed to fetch previous photo:', e);
                }
            };
        }

        const nextBtn = popover.querySelector('#next_button');
        if (nextBtn) {
            nextBtn.onclick = async () => {
                if (!this._next) return;
                try {
                    const r = await fetch(`/photo_scroll/${this._next}/${this._way_ids}`);
                    if (!r.ok) return;
                    const data = await r.json();
                    let newPhotoScroll = new ViPhotoScroll(data);
                    document.getElementById('photo_scroll_inner').remove();
                    document.body.appendChild(newPhotoScroll);
                } catch (e) {
                    console.error('Failed to fetch next photo:', e);
                }
            };
        }
    }

    connectedCallback() {
        // ensure the popover is appended and listeners attached when element connects
        this._attachListeners();
    }
}

customElements.define('vi-photo-scroll', ViPhotoScroll, {});