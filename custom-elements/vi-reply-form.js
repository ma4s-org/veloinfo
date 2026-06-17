import { getViMain } from '/custom-elements/vi-context.js';

let html = String.raw;

/**
 * Cherche récursivement un commentaire par son ID dans une arbre de réponses.
 * Retourne le texte du commentaire ou null.
 */
function findCommentText(replies, id) {
    for (let reply of replies) {
        if (reply.id == id) return reply.comment;
        if (reply.replies?.length > 0) {
            let found = findCommentText(reply.replies, id);
            if (found) return found;
        }
    }
    return null;
}

/**
 * Recharge les contributions de la zone et met à jour le composant vi-info.
 */
async function reloadContributions() {
    let bounds = getViMain().map.getBounds();
    let r = await fetch(`/info_panel/up/${bounds._sw.lng}/${bounds._sw.lat}/${bounds._ne.lng}/${bounds._ne.lat}`);
    let json = await r.json();
    let viInfo = document.querySelector('vi-info');
    if (viInfo) viInfo.data = json;
}

/**
 * Crée et affiche le formulaire de réponse dans #info_panel_up.
 *
 * @param {string} reportId - L'ID du report
 * @param {string|null} parentCommentId - L'ID du commentaire parent (ou null pour une réponse au commentaire racine)
 * @param {function} onSuccess - Callback appelé après une soumission réussie
 */
export async function showReplyForm(reportId, parentCommentId, onSuccess) {
    const infoPanelUp = document.querySelector('#info_panel_up');
    if (!infoPanelUp) return;

    // Récupérer les informations du report
    let r = await fetch(`/segment_panel/id/${reportId}`, { credentials: 'same-origin' });
    let data = await r.json();

    // Déterminer le texte du commentaire auquel on répond
    let commentToReplyTo = data.comment || 'Aucun commentaire';
    if (parentCommentId && parentCommentId !== '0') {
        let bounds = getViMain().map.getBounds();
        let commentsRes = await fetch(`/info_panel/up/${bounds._sw.lng}/${bounds._sw.lat}/${bounds._ne.lng}/${bounds._ne.lat}`);
        let commentsData = await commentsRes.json();
        for (let contrib of commentsData.contributions || []) {
            let found = findCommentText(contrib.replies || [], parseInt(parentCommentId));
            if (found) {
                commentToReplyTo = found;
                break;
            }
        }
    }

    const userName = data.user_name || '';

    const replyForm = document.createElement('div');
    replyForm.style.cssText = 'position: absolute; width: 100%; height: 100%; background: white; overflow: auto;';
    replyForm.innerHTML = html`
        <div style="padding: 0.5rem; margin: 0.25rem;">
            <h3 style="margin-bottom: 1rem; text-transform: uppercase;">Répondre au commentaire</h3>
            <div style="margin-bottom: 1rem; padding: 0.5rem; background: #f3f4f6; border-radius: 0.5rem; border-left: 3px solid #2563eb;">
                <div style="font-size: small; color: #6b7280; font-weight: bold;">Commentaire original :</div>
                <div style="font-size: small; margin-top: 0.25rem; font-style: italic;">"${commentToReplyTo || 'Aucun commentaire'}"</div>
                ${data.photo_path_thumbnail && data.photo_path_thumbnail !== "null" ?
                    `<img style="width: 4rem; height: 4rem; margin-top: 0.5rem; border-radius: 0.25rem;" src="${data.photo_path_thumbnail}" alt="photo">` : ''}
            </div>
            <form id="reply_form">
                <input type="hidden" name="report_id" value="${reportId}">
                ${parentCommentId ? `<input type="hidden" name="parent_comment_id" value="${parentCommentId}">` : ''}
                <input type="text" name="user_name" style="border: 2px solid; border-color: #80808099; width: 100%; margin-bottom: 0.5rem;" placeholder="Nom" value="${userName}">
                <textarea rows="4" name="comment" style="border: 2px solid; border-color: #80808099; width: 100%; margin-bottom: 0.5rem;" placeholder="Votre commentaire"></textarea>
                <div style="text-transform: uppercase; margin: 0.5rem;">
                    <label for="photo">Choisissez une photo :</label>
                    <input type="file" id="photo" name="photo" accept="image/*">
                </div>
                <div style="display: flex; justify-content: center; gap: 0.5rem; margin-top: 1rem;">
                    <md-filled-button id="cancel_reply" type="button">Annuler</md-filled-button>
                    <md-filled-button id="submit_reply" type="button">Envoyer</md-filled-button>
                </div>
            </form>
        </div>
    `;

    infoPanelUp.innerHTML = '';
    infoPanelUp.appendChild(replyForm);

    // Annuler
    replyForm.querySelector('#cancel_reply')?.addEventListener('click', async (e) => {
        e.stopImmediatePropagation();
        e.preventDefault();
        await reloadContributions();
    });

    // Envoyer
    const form = replyForm.querySelector('#reply_form');
    replyForm.querySelector('#submit_reply')?.addEventListener('click', async (e) => {
        e.stopImmediatePropagation();
        e.preventDefault();

        if (!form.checkValidity()) {
            form.reportValidity();
            return;
        }

        const formData = new FormData(form);

        try {
            const response = await fetch('/report/reply', {
                method: 'POST',
                body: formData,
                credentials: 'same-origin'
            });
            const result = await response.json();

            if (result.success) {
                if (onSuccess) {
                    onSuccess(result);
                } else {
                    await reloadContributions();
                }
            } else {
                alert('Erreur: ' + (result.error || 'Erreur inconnue'));
            }
        } catch (err) {
            alert('Erreur de connexion: ' + err.message);
        }
    });
}