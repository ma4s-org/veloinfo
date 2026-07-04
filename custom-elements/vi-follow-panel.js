import { getViMain } from '/custom-elements/vi-context.js';

class FollowPanel extends HTMLElement {
    constructor() {
        super();
        // Only safe initializations allowed in constructor (no DOM/attribute access)
        this.routeCoordinates = null;
        // Utilisation d'une variable privée pour supporter le getter/setter
        this._routeNames = null;
        // Track the last returned coordinate index
        this.lastPassedVertex = null;
    }

    set routeNames(value) {
        let route = this.getAttribute('route');
        if (this.isConnected && value) {
            // Tableau 2D initial ([ [safe], [fast] ]) ou 1D (recalcul WebSocket)
            if (Array.isArray(value) && Array.isArray(value[0])) {
                this._routeNames = (route === "safe") ? (value[0] || []) : (value[1] || []);
            } else {
                this._routeNames = value || [];
            }
        } else {
            // Stocké temporairement en attendant l'exécution de connectedCallback
            this._routeNames = value;
        }
    }

    get routeNames() {
        return this._routeNames || [];
    }

    connectedCallback() {
        // Solution 1 : Déclaré localement ici pour éviter l'erreur 2451 (conflit de scope)
        const html = String.raw;

        // Read attributes and set up state now that the element is connected
        let map = getViMain().map;
        let coordinates = JSON.parse(this.getAttribute('coordinates'));
        let route = this.getAttribute('route');
        let routeIndex = route === "safe" ? 0 : 1;
        let otherId = route === "safe" ? "selected_fast" : "selected_safe";
        let rawNamesAttr = this.getAttribute('route_names');
        let names = rawNamesAttr ? JSON.parse(rawNamesAttr) : null;

        this.routeCoordinates = coordinates[routeIndex];
        if (rawNamesAttr) {
            this.routeNames = names;
        } else if (this._routeNames) {
            // Pré-assigné par propriété avant connexion (tableau 2D non traité)
            this._routeNames = (Array.isArray(this._routeNames) && Array.isArray(this._routeNames[0]))
                ? (this._routeNames[routeIndex] || [])
                : (this._routeNames || []);
        }
        if (map.getLayer(otherId)) {
            map.removeLayer(otherId);
        }
        if (map.getSource(otherId)) {
            map.removeSource(otherId);
        }

        let totalDistance = getViMain().calculateTotalDistance(this.routeCoordinates, 0).toFixed(1);
        let innerHTML = html`
            <div class="vi-panel">
                <div id="follow" style="display: flex; flex-direction: column; justify-content: center;">
                    <div id="next_street" style="display: flex; flex-direction: row; align-items: baseline; justify-content: center; padding: 0.5em 1em; min-height: 2em; font-size: 1.05em; font-weight: bold; color: #1a73e8; line-height: 1.4; text-align: center;">
                    </div>
                    <div style="display: flex;justify-content: center;">
                        <div>
                            distance à faire :
                        </div>
                        <div id="total_distance" style="margin-left: 2em; font-size: 1.2em; font-weight: bold;">
                            ${totalDistance} kms
                        </div>
                    </div>
                    <div style="display: flex;justify-content: center;">
                        <md-filled-button id="cancel-follow-btn">annuler</md-filled-button>
                    </div>
                </div>
            </div>
        `;
        this.innerHTML = innerHTML;
        this.querySelector('#cancel-follow-btn').addEventListener('click', () => {
            getViMain().clear();
        });

        this.intervalId = setInterval(async () => {
            if (!document.body.contains(this)) {
                clearInterval(this.intervalId);
                return;
            }
            this.updatePosition();
        }, 5_000);
        this.updatePosition();
    }

    disconnectedCallback() {
        if (getViMain().isGeolocateActive) {
            getViMain().geolocate.trigger();
        }
        clearInterval(this.intervalId);
    }

    updatePosition() {
        if (this.updating) {
            return;
        }
        navigator.geolocation.getCurrentPosition(async (position) => {

            let lastPassedVertexIndex = this.findLastPassedVertex(
                position.coords.longitude,
                position.coords.latitude,
                this.routeCoordinates
            );
            let distanceToRoute = this.distanceToRoute(
                position.coords.longitude,
                position.coords.latitude,
                this.routeCoordinates,
                lastPassedVertexIndex
            );

            if (distanceToRoute > 0.05) { // 50 meters
                // we are too far from the route. We calculate it again.
                this.updating = true;
                const socket = new WebSocket(`/recalculate_route/${this.getAttribute('route')}/${position.coords.longitude}/${position.coords.latitude}/${this.routeCoordinates[this.routeCoordinates.length - 1][0]}/${this.routeCoordinates[this.routeCoordinates.length - 1][1]}?allow_ferry=true`);
                socket.onerror = () => { this.updating = false; };
                socket.onclose = (event) => {
                    if (event.code !== 1000 && this.updating) {
                        this.updating = false;
                    }
                };
                socket.onmessage = async (event) => {
                    let data = JSON.parse(event.data);
                    if (data.coordinates) {
                        socket.close();
                        let sourceId = this.getAttribute('route') === "safe" ? "selected_safe" : "selected_fast";
                        getViMain().map.getSource(sourceId).setData({
                            "type": "Feature",
                            "properties": {},
                            "geometry": {
                                "type": "MultiLineString",
                                "coordinates": [data.coordinates]
                            }
                        });
                        this.routeCoordinates = data.coordinates;
                        this.lastPassedVertex = null;
                        this.routeNames = data.names || [];
                        this.updating = false;
                        // Rejoue updatePosition avec la nouvelle route.
                        this.updatePosition();
                        return;
                    } else {
                        // non-coordinate message; ignore
                    }
                }
                return;
            }
            let totalDistance = getViMain().calculateTotalDistance(this.routeCoordinates, lastPassedVertexIndex).toFixed(1);
            if (document.getElementById('total_distance')) {
                document.getElementById('total_distance').innerText = `${totalDistance} kms`;
            }
            this.updateNextStreet(lastPassedVertexIndex, position.coords.longitude, position.coords.latitude);
            this.setBearing(this.routeCoordinates, position.coords.latitude, position.coords.longitude, lastPassedVertexIndex);
        });
    }

    updateNextStreet(lastPassedVertexIndex, currentLng, currentLat) {
        let nextStreetEl = document.getElementById('next_street');
        if (!nextStreetEl) {
            return;
        }
        if (!this.routeNames || this.routeNames.length === 0 || !this.routeCoordinates) {
            nextStreetEl.innerText = '';
            return;
        }

        // Sécurité : si routeNames et routeCoordinates n'ont pas la même longueur,
        // on ne peut pas faire confiance aux index pour les virages.
        if (this.routeNames.length > this.routeCoordinates.length) {
            nextStreetEl.innerText = this.routeNames[lastPassedVertexIndex] || 'route inconnue';
            return;
        }

        // names[i] = nom de l'edge i (segment [i, i+1]).
        // Le vertex 0 (raccordement) a souvent name = null, on décale alors
        // au nom du segment suivant.
        let currentName = this.routeNames[lastPassedVertexIndex]
            || this.routeNames[lastPassedVertexIndex + 1]
            || 'route inconnue';

        // Trouver le prochain index où le nom change (le "turn index")
        // Un nom null est un changement de rue (ruelle sans nom, chemin, etc.)
        let turnIndex = -1;
        let nextName = null;
        for (let i = lastPassedVertexIndex + 1; i < this.routeNames.length; i++) {
            let name = this.routeNames[i];
            if (name !== currentName) {
                turnIndex = i;
                nextName = name || 'route inconnue';
                break;
            }
        }

        if (turnIndex === -1) {
            // Pas de virage à venir, on est sur la dernière rue
            nextStreetEl.innerText = currentName;
            return;
        }

        // Calculer la distance jusqu'au point de virage (l'intersection)
        let distanceToTurn = this.calculateDistance(
            currentLat, currentLng,
            this.routeCoordinates[turnIndex - 1][1], this.routeCoordinates[turnIndex - 1][0]
        );

        // N'afficher la prochaine rue que si le virage est à moins de 300 mètres
        if (distanceToTurn <= 0.3) {
            let coords = this.routeCoordinates;
            let turnDirection = this.getTurnDirection(coords, turnIndex);
            let arrow = this.getTurnArrow(turnDirection);
            let distanceMeters = Math.round(distanceToTurn * 1000);
            nextStreetEl.innerHTML = `${currentName} <span style="font-size: 2.5em; line-height: 0; vertical-align: middle;">${arrow}</span> ${nextName} (${distanceMeters} m)`;
        } else {
            nextStreetEl.innerText = currentName;
        }
    }

    getTurnDirection(coords, turnIndex) {
        if (!coords || turnIndex < 1 || turnIndex >= coords.length) {
            return 'straight';
        }
        let viMain = getViMain();

        // Bearing avant le virage : calculé sur 2 points en amont
        // (au lieu d'un seul) pour lisser les segments OSM très courts
        // sans risquer de traverser une intersection.
        let lookBack = Math.min(2, turnIndex);
        let bearingBefore = viMain.calculateBearing(
            coords[turnIndex - lookBack][0], coords[turnIndex - lookBack][1],
            coords[turnIndex - 1][0], coords[turnIndex - 1][1]
        );

        // Bearing après le virage : 2 points en aval pour capter
        // la direction réelle de la nouvelle rue.
        let lookForward = Math.min(2, coords.length - turnIndex);
        let bearingAfter = viMain.calculateBearing(
            coords[turnIndex - 1][0], coords[turnIndex - 1][1],
            coords[turnIndex + lookForward - 1][0], coords[turnIndex + lookForward - 1][1]
        );

        // Différence angulaire normalisée entre -180 et +180
        let diff = bearingAfter - bearingBefore;
        if (diff > 180) diff -= 360;
        if (diff < -180) diff += 360;

        // Seuils : |diff| < 30 = tout droit, < -135 = demi-tour gauche,
        // < -30 = gauche, < 135 = droite, sinon demi-tour droite
        if (Math.abs(diff) < 30) {
            return 'straight';
        } else if (diff < -135) {
            return 'uturn-left';
        } else if (diff < -30) {
            return 'left';
        } else if (diff < 135) {
            return 'right';
        } else {
            return 'uturn-right';
        }
    }

    getTurnArrow(direction) {
        switch (direction) {
            case 'left': return '←';
            case 'right': return '→';
            case 'straight': return '↑';
            case 'uturn-left': return '⤺';
            case 'uturn-right': return '⤻';
            default: return '→';
        }
    }

    findLastPassedVertex(longitude, latitude, coordinates) {
        // On suit le dernier vertex de l'itinéraire que l'on a franchi.
        // On n'avance au vertex suivant que lorsqu'on l'a physiquement
        // dépassé : la projection sur le segment courant dépasse la fin
        // (t >= 1), ou on est à moins de 15 m du vertex (bruit GPS).
        // On ne recule jamais : une fois un vertex passé, il l'est pour de bon.
        let currentIndex = this.lastPassedVertex ?? 0;
        while (currentIndex < coordinates.length - 1) {
            let t = this.projectOnSegment(
                longitude, latitude,
                coordinates[currentIndex][0], coordinates[currentIndex][1],
                coordinates[currentIndex + 1][0], coordinates[currentIndex + 1][1]
            );
            let distToNextKm = this.calculateDistance(
                latitude, longitude,
                coordinates[currentIndex + 1][1], coordinates[currentIndex + 1][0]
            );
            if (t >= 1 || distToNextKm < 0.015) {
                currentIndex++;
            } else {
                break;
            }
        }
        this.lastPassedVertex = currentIndex;
        return currentIndex;
    }

    calculateDistance(lat1, lon1, lat2, lon2) {
        const R = 6371; // Earth's radius in km (result is in km)
        const φ1 = lat1 * Math.PI / 180;
        const φ2 = lat2 * Math.PI / 180;
        const Δφ = (lat2 - lat1) * Math.PI / 180;
        const Δλ = (lon2 - lon1) * Math.PI / 180;

        const a = Math.sin(Δφ / 2) * Math.sin(Δφ / 2) +
            Math.cos(φ1) * Math.cos(φ2) *
            Math.sin(Δλ / 2) * Math.sin(Δλ / 2);
        const c = 2 * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a));

        const distance = R * c;
        return distance;
    }

    /**
     * Distance perpendiculaire du point (lng, lat) au segment de route le plus
     * proche (segments autour de lastPassedVertexIndex). On utilise la distance
     * à la droite portant le segment (non clamped) : pertinente même quand la
     * projection tombe hors du segment.
     */
    distanceToRoute(lng, lat, coords, lastPassedVertexIndex) {
        if (!coords || coords.length === 0) return Infinity;

        let minDistance = Infinity;
        let segments = [];
        if (lastPassedVertexIndex > 0) {
            segments.push([lastPassedVertexIndex - 1, lastPassedVertexIndex]);
        }
        if (lastPassedVertexIndex < coords.length - 1) {
            segments.push([lastPassedVertexIndex, lastPassedVertexIndex + 1]);
        }

        for (let [i, j] of segments) {
            let d = this.perpendicularDistanceToLine(
                lng, lat,
                coords[i][0], coords[i][1],
                coords[j][0], coords[j][1]
            );
            if (d < minDistance) {
                minDistance = d;
            }
        }

        if (segments.length === 0) {
            return this.calculateDistance(lat, lng, coords[0][1], coords[0][0]);
        }

        return minDistance;
    }

    /**
     * Hauteur du triangle rectangle formé par le point P et le segment [A, B],
     * i.e. la distance perpendiculaire de P à la droite portant [A, B].
     * Formule : h = |AP × AB| / |AB|  (produit vectoriel 2D).
     * Retourne la distance en km.
     */
    perpendicularDistanceToLine(px, py, ax, ay, bx, by) {
        const latRad = py * Math.PI / 180;
        const cosLat = Math.cos(latRad);
        const kmPerDegLng = 111.32 * cosLat;
        const kmPerDegLat = 110.574;

        let pxk = px * kmPerDegLng;
        let pyk = py * kmPerDegLat;
        let axk = ax * kmPerDegLng;
        let ayk = ay * kmPerDegLat;
        let bxk = bx * kmPerDegLng;
        let byk = by * kmPerDegLat;

        let abx = bxk - axk;
        let aby = byk - ayk;
        let abLen = Math.sqrt(abx * abx + aby * aby);

        if (abLen === 0) {
            // A et B confondus : distance au point A
            let dx = pxk - axk;
            let dy = pyk - ayk;
            return Math.sqrt(dx * dx + dy * dy);
        }

        // Produit vectoriel 2D : |AP × AB| = |apx * aby - apy * abx|
        let apx = pxk - axk;
        let apy = pyk - ayk;
        let cross = Math.abs(apx * aby - apy * abx);

        return cross / abLen;
    }

    /**
     * Retourne le paramètre t de la projection de (px, py) sur le segment [A, B].
     * t = 0 → point A, t = 1 → point B, t > 0.5 → on a passé le milieu du segment.
     */
    projectOnSegment(px, py, ax, ay, bx, by) {
        const latRad = py * Math.PI / 180;
        const cosLat = Math.cos(latRad);
        const kmPerDegLng = 111.32 * cosLat;
        const kmPerDegLat = 110.574;

        let pxk = px * kmPerDegLng;
        let pyk = py * kmPerDegLat;
        let axk = ax * kmPerDegLng;
        let ayk = ay * kmPerDegLat;
        let bxk = bx * kmPerDegLng;
        let byk = by * kmPerDegLat;

        let abx = bxk - axk;
        let aby = byk - ayk;
        let abLenSq = abx * abx + aby * aby;

        if (abLenSq === 0) return 1; // A et B confondus : considère qu'on a passé A

        let apx = pxk - axk;
        let apy = pyk - ayk;
        let t = (apx * abx + apy * aby) / abLenSq;
        return t;
    }

    setBearing(coordinates, currentLat, currentLng, startIndex) {
        if (!document.body.contains(this)) {
            return;
        }

        // Find the first coordinate that is at least 50 meters away from current position,
        // en cherchant vers l'avant à partir du point suivant l'index courant.
        let hundredMeterAwayIndex = coordinates
            .slice(startIndex + 1)
            .findIndex(coord =>
                this.calculateDistance(
                    currentLat,
                    currentLng,
                    coord[1],
                    coord[0]
                ) >= 0.05 // 50 meters
            );
        
        let targetIndex;
        let useSegmentBearing = false;

        if (hundredMeterAwayIndex === -1) {
            // Aucun point à 50m devant. On est en fin d'itinéraire :
            // on utilise toujours la direction du dernier segment.
            targetIndex = coordinates.length - 1;
            useSegmentBearing = true;
        } else {
            targetIndex = hundredMeterAwayIndex + startIndex + 1;
        }

        let bearing;
        if (useSegmentBearing) {
            // Utiliser la direction du dernier segment de l'itinéraire
            let p1 = coordinates[Math.max(0, targetIndex - 1)];
            let p2 = coordinates[targetIndex];
            bearing = getViMain().calculateBearing(p1[0], p1[1], p2[0], p2[1]);
        } else {
            bearing = getViMain().calculateBearing(
                currentLng,
                currentLat,
                coordinates[targetIndex][0],
                coordinates[targetIndex][1]
            );
        }

        let map = getViMain().map;
        map.easeTo({
            pitch: 60,
            bearing,
            duration: 1_600,
        });
        setTimeout(() => {
            if (!document.body.contains(this)) {
                return;
            }
            if (!getViMain().isGeolocateActive) {
                getViMain().geolocate.trigger();
            }
        }, 1_600);
    }

}

customElements.define("vi-follow-panel", FollowPanel); 
