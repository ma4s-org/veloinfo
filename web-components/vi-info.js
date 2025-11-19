class ViInfo extends HTMLElement {
  constructor() {
    super();
  }

  set data(data) {
    this.render(data);
  }

  render(data) {
    this.innerHTML = data?.arrow === "▼" ?
      `<div id="info_panel_up" style="position: absolute; width: 100%; height: 40%; max-width: 500px; background-color: white; z-index: 10; bottom: 0; border-radius: 0.5rem;">
        <img id="spinner" style="z-index: 30; bottom: 2rem; margin-left: auto; margin-right: auto; left: 0; right: 0;" class="htmx-indicator" src="/pub/bars.svg">
        <div id="info_panel_up_header" style="width: 100%; height: 1.75rem; display: flex; justify-content: center; cursor: pointer;">
            <div style="text-transform: uppercase; font-weight: bold;">Contributions dans cette zone</div>
            <div style="position: absolute; right: 0.5rem; top: 0;">
                ▼
            </div>
        </div>
        <div style="overflow: auto; height: 100%;">
          ${data.contributions?.map(contribution => /*html*/`
                            <div style="padding: 0.5rem;">
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
      </div>` :
      `<div id="info_panel_down" style="position: absolute; height: 3rem; width: 100%; max-width: 500px; background-color: white; z-index: 10; bottom: 0; border-radius: 0.5rem;">     
        <img id="spinner" style="z-index: 30; bottom: 2rem; margin-left: auto; margin-right: auto; left: 0; right: 0;" class="htmx-indicator" src="/pub/bars.svg">
        <div id="info_panel_down_header" style="width: 100%; height: 1.75rem; display: flex; justify-content: center; cursor: pointer;">
            <div style="text-transform: uppercase; font-weight: bold;">Contributions dans cette zone</div>
            <div style="position: absolute; right: 0.5rem;">
                ▲
            </div>
        </div>
        <div class="overflow-auto h-full">
            <hr>
        </div>
       </div>`;
    this.querySelector("#info_panel_up_header")?.addEventListener("click", async () => {
      let r = await fetch("/info_panel/down");
      let json = await r.json();
      this.data = json;
    });
    this.querySelector("#info_panel_down_header")?.addEventListener("click", async () => {
      let bounds = document.querySelector('veloinfo-map').map.getBounds();
      let r = await fetch("/info_panel/up/" + bounds._sw.lng + "/" + bounds._sw.lat + "/" + bounds._ne.lng + "/" + bounds._ne.lat);
      let json = await r.json();
      this.data = json;
    });

  }
}

customElements.define('vi-info', ViInfo);