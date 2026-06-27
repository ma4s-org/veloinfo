import { getViMain } from '/custom-elements/vi-context.js';
import { showReplyForm } from '/custom-elements/vi-reply-form.js';
let html = String.raw;

// Fonction récursive pour afficher les réponses avec indentation progressive
function renderReplies(replies, report_id, depth, parentId) {
    if (!replies || replies.length === 0) return '';
    
    const indent = depth * 2; // 2rem par niveau
    
    return replies.map(reply => html`
        <div style="padding: 0.5rem; margin-left: ${indent}rem; border-left: 2px solid #2563eb; margin-top: 0.5rem; display: flex; justify-content: space-between; align-items: flex-start;">
            <div style="flex: 1;">
                <div style="font-size: small; color: #6b7280;">
                    <span style="font-weight: bold;">${reply.user_name || 'Anonyme'}</span>
                    <span style="margin-left: 0.5rem;">${reply.timeago}</span>
                </div>
                <div style="margin-top: 0.25rem; font-size: small; display: flex; flex-direction: row; align-items: center;">
                    ${reply.photo_path_thumbnail && reply.photo_path_thumbnail !== "null" ? `<img style="width: 2rem; height: 2rem; margin: 0 0.5rem; border-radius: 0.125rem;" src="${reply.photo_path_thumbnail}" alt="photo">` : ''}
                    <div>${reply.comment}</div>
                </div>
            </div>
            <md-icon-button id="reply_${report_id}_${reply.id}" style="--md-icon-button-icon-size: 1.25rem;">
                <span class="material-icons" style="font-size: 1.25rem;">reply</span>
            </md-icon-button>
        </div>
        ${reply.replies?.length > 0 ? html`
            <div style="margin-top: 0.5rem;">
                ${renderReplies(reply.replies, report_id, depth + 1)}
            </div>
        ` : ''}
    `).join('');
}

export default class ViInfo extends HTMLElement {
  constructor(data) {
    super();
    this.render(data);
  }

  set data(data) {
    this.render(data);
  }

  render(data) {
    const that = this;
    const allDisabled = data?.contributions?.length > 0 && data.contributions.every(c => c.enabled === false || c.enabled === "false");
    const showUp = data?.arrow === "▼" && (!allDisabled || data?.forceUp);
    this.innerHTML = showUp ?
      html`<div id="info_panel_up" style="position: absolute; width: 100%; height: 40%; max-width: 500px; background-color: white; z-index: 10; bottom: 0; border-radius: 0.5rem; display: flex; flex-direction: column;">
        <img id="spinner" style="z-index: 30; bottom: 2rem; margin-left: auto; margin-right: auto; left: 0; right: 0; display: none;" src="/pub/bars.svg">
        <div id="info_panel_up_header" style="width: 100%; height: 1.75rem; flex-shrink: 0; display: flex; justify-content: center; cursor: pointer;">
            <div style="text-transform: uppercase; font-weight: bold;">Contributions dans cette zone</div>
            <div style="position: absolute; right: 0.5rem; top: 0;">
                ▼
            </div>
        </div>
        <div style="overflow: auto; flex: 1;">
          ${data.contributions?.map(contribution => html`
            <div style="padding: 0.5rem; border-bottom: 1px solid #e5e7eb;">
              <infopanel-contribution
                  comment-id="${contribution.id || ''}"
                  created_at="${contribution.created_at}"
                  timeago="${contribution.timeago}"
                  name="${contribution.name}"
                  photo_path_thumbnail="${contribution.photo_path_thumbnail}"
                  report_id="${contribution.report_id}"
                  score="${contribution.score_circle.score}"
                  user_name="${contribution.user_name}"
                  timestamp="${contribution.timestamp}"
                  enabled="${contribution.enabled}"
                  comment="${contribution.comment}">
              </infopanel-contribution>
              ${contribution.replies?.length > 0 ? html`
                <div style="margin-top: 0.5rem;">
                  ${renderReplies(contribution.replies, contribution.report_id, 1, contribution.report_id)}
                </div>
              ` : ''}
            </div>
          `).join('')}

        </div>
      </div>` :
      html`<div id="info_panel_down" style="position: absolute; height: 3rem; width: 100%; max-width: 500px; background-color: white; z-index: 10; bottom: 0; border-radius: 0.5rem;">     
        <img id="spinner" style="z-index: 30; bottom: 2rem; margin-left: auto; margin-right: auto; left: 0; right: 0; display: none;" src="/pub/bars.svg">
        <div id="info_panel_down_header" style="width: 100%; height: 1.75rem; display: flex; justify-content: center; cursor: pointer;">
            <div style="text-transform: uppercase; font-weight: bold;">Contributions dans cette zone</div>
            <div style="position: absolute; right: 0.5rem;">
                ▲
            </div>
        </div>
        <div style="overflow: auto; height: 100%;">
            <hr>
        </div>
       </div>`;
    
    // Gestion des clicks sur les boutons Reply des réponses imbriquées (renderReplies)
    // Les boutons reply des contributions racine sont gérés par InfopanelContribution.connectedCallback
    this.querySelectorAll('[id^="reply_"]:not(infopanel-contribution [id^="reply_"])').forEach(btn => {
      btn.addEventListener('click', async (e) => {
        e.stopImmediatePropagation();
        e.preventDefault();
        const parts = btn.id.replace('reply_', '').split('_');
        const report_id = parts[0];
        const parent_comment_id = parts[1] || null;
        await showReplyForm(report_id, parent_comment_id);
      });
    });

    this.querySelector("#info_panel_up_header")?.addEventListener("click", async () => {
      this.data = null;
    });
    this.querySelector("#info_panel_down_header")?.addEventListener("click", async () => {
      let bounds = getViMain().map.getBounds();
      let r = await fetch("/info_panel/up/" + bounds._sw.lng + "/" + bounds._sw.lat + "/" + bounds._ne.lng + "/" + bounds._ne.lat);
      let json = await r.json();
      this.data = { ...json, forceUp: true };
    });

  }
}

customElements.define('vi-info', ViInfo);