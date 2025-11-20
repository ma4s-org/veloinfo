class ViMobilizonEvents extends HTMLElement {
    constructor() {
        super();
        this.attachShadow({ mode: 'open' });
    }

    async connectedCallback() {
        const events = await this.fetchEvents();
        this.render(events);
    }


    render(events) {
    }

    async fetchEvents() {
        const apiUrl = 'https://evenement.facil.services/api';

        const graphqlQuery = `
            query SearchEvents($beginsOn: DateTime, $endsOn: DateTime) {
            searchEvents(
                beginsOn: $beginsOn,
                endsOn: $endsOn,
                tags: "vélo"
            ) {
                total
                elements {
                id
                title
                uuid
                beginsOn
                endsOn
                picture {
                    url
                }
                physicalAddress {
                    street
                    locality
                    geom
                }
                }
            }
            }
        `;

        const today = new Date();
        const fourWeekFromNow = new Date();
        fourWeekFromNow.setDate(today.getDate() + 28);

        const variables = {
            // Convertit les dates au format ISO 8601 requis par l'API (ex: "2025-09-09T22:25:17.123Z")
            beginsOn: today.toISOString(),
            endsOn: fourWeekFromNow.toISOString()
        };

        try {
            const response = await fetch(apiUrl, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({
                    query: graphqlQuery,
                    variables: variables
                })
            });

            // Extraire les données JSON de la réponse
            const result = await response.json();

            // Gérer les erreurs retournées par l'API GraphQL elle-même
            if (result.errors) {
                console.error("Erreurs de l'API GraphQL:", result.errors);
                return;
            }
            const events = result.data.searchEvents.elements;
            //Afficher les événements sur la carte
            events.forEach(event => {
                if (event.physicalAddress && event.physicalAddress.geom) {
                    const coords = event.physicalAddress.geom.split(";").map(Number);
                    // Utilise une icône SVG de calendrier comme élément HTML pour le marqueur
                    // Affiche le jour du mois dans l'icône SVG
                    const beginsDate = new Date(event.beginsOn);
                    const dayOfMonth = beginsDate.getDate();

                    const calendarIcon = `
                        <svg id="calendar-icon" width="42" height="42" viewBox="0 0 42 42" fill="none">
                            <rect x="5.25" y="10.5" width="31.5" height="26.25" rx="5.25" fill="#fff" stroke="#1976d2" stroke-width="2.625"/>
                            <rect x="5.25" y="10.5" width="31.5" height="5.25" fill="#1976d2"/>
                            <rect x="13.125" y="2.625" width="5.25" height="10.5" rx="2.625" fill="#1976d2"/>
                            <rect x="23.625" y="2.625" width="5.25" height="10.5" rx="2.625" fill="#1976d2"/>
                            <text x="21" y="28.875" text-anchor="middle" alignment-baseline="middle" fill="#1976d2" font-size="15.75" font-family="Arial">${dayOfMonth}</text>
                        </svg>
                    `;

                    const el = document.createElement('div');
                    el.innerHTML = calendarIcon;
                    el.style.width = '32px';
                    el.style.height = '32px';

                    // Format la date et l'heure comme "14 septembre à 11h"
                    const options = { day: 'numeric', month: 'long' };
                    const dateStr = beginsDate.toLocaleDateString('fr-FR', options);
                    const hourStr = beginsDate.getHours() + "h";
                    const formattedDate = `${dateStr} à ${hourStr}`;
                    let map = document.querySelector('vi-main').map;
                    const marker = new maplibregl.Marker({ element: el })
                        .setLngLat([coords[0], coords[1]])
                        .setPopup(new maplibregl.Popup({ offset: 25 })
                            .setHTML(`
                                    <div>
                                        <h3><a href="https://evenement.facil.services/events/${event.uuid}">${event.title}</a></h3>
                                        <p><strong>Quand:</strong> ${formattedDate}</p>
                                        <p><strong>Où:</strong> ${event.physicalAddress.street || ''}, ${event.physicalAddress.municipality || event.physicalAddress.locality || ''}</p>
                                        ${event.picture ? `<img src="${event.picture.url}" alt="${event.title}" style="width:100%;height:auto;"/>` : ''}
                                        <div style="display: flex; justify-content: center; margin-top: 8px;">
                                            <md-filled-button id="route_md-filled-button" >
                                                <img slot="icon" src="/pub/directions.png" class="w-4 h-4 mr-1 ">itinéraire
                                            </md-filled-button>
                                        </div>
                                    </div>`)
                        )
                        .addTo(map);
                    var listener = (e) => {
                        if (window.start_marker) {
                            window.start_marker.remove();
                            window.start_marker = null;
                        } window.start_marker = new window.maplibregl.Marker({ color: "#00f" }).setLngLat(coords).addTo(map);
                        marker.getPopup().remove();
                        document.querySelector('vi-main').route();
                    }
                    marker.getPopup().on('open', () => {
                        const btn = marker.getPopup()._content.querySelector('#route_md-filled-button');
                        btn.addEventListener('click', listener);
                    });
                    marker.getPopup().on('close', () => {
                        const btn = marker.getPopup()._content.querySelector('#route_md-filled-button');
                        btn.removeEventListener('click', listener);
                    });
                    marker.getElement().addEventListener('click', (e) => {
                        marker.togglePopup();
                        e.stopPropagation();
                    });
                }
            });

        } catch (error) {
            console.error("❌ Impossible de récupérer les événements :", error);
        }
    }
}

customElements.define('vi-mobilizon-events', ViMobilizonEvents);