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
        <img id="spinner" class="htmx-indicator z-30 bottom-8 mx-auto inset-x-0" src="/pub/bars.svg">
        <div id="info_panel_up_header" style="width: 100%; height: 1.75rem; display: flex; justify-content: center; cursor: pointer;">
            <div class="uppercase font-bold">Contributions dans cette zone</div>
            <div class="absolute right-2">
                ▼
            </div>
        </div>
        <div class="overflow-auto h-full">
          ${data.contributions?.map(contribution => /*html*/`
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
      </div>` :
      `<div id="info_panel_down" style="position: absolute; height: 3rem; width: 100%; max-width: 500px; background-color: white; z-index: 10; bottom: 0; border-radius: 0.5rem;">     
        <img id="spinner" class="htmx-indicator z-30 bottom-8 mx-auto inset-x-0" src="/pub/bars.svg">
        <div id="info_panel_down_header" style="width: 100%; height: 1.75rem; display: flex; justify-content: center; cursor: pointer;">
            <div class="uppercase font-bold">Contributions dans cette zone</div>
            <div class="absolute right-2">
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