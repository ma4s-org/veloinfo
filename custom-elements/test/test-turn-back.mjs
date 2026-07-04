function calculateBearing(lon1, lat1, lon2, lat2) {
    lon1 *= Math.PI / 180.0;
    lat1 *= Math.PI / 180.0;
    lon2 *= Math.PI / 180.0;
    lat2 *= Math.PI / 180.0;
    const y = Math.sin(lon2 - lon1) * Math.cos(lat2);
    const x = Math.cos(lat1) * Math.sin(lat2) - Math.sin(lat1) * Math.cos(lat2) * Math.cos(lon2 - lon1);
    let bearing = Math.atan2(y, x) * (180 / Math.PI);
    return (bearing + 360) % 360;
}

function calculateDistance(lat1, lon1, lat2, lon2) {
    const R = 6371;
    const φ1 = lat1 * Math.PI / 180;
    const φ2 = lat2 * Math.PI / 180;
    const Δφ = (lat2 - lat1) * Math.PI / 180;
    const Δλ = (lon2 - lon1) * Math.PI / 180;
    const a = Math.sin(Δφ / 2) ** 2 + Math.cos(φ1) * Math.cos(φ2) * Math.sin(Δλ / 2) ** 2;
    const c = 2 * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a));
    return R * c;
}

function projectOnSegment(px, py, ax, ay, bx, by) {
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

    let dx = bxk - axk;
    let dy = byk - ayk;
    let segLenSq = dx * dx + dy * dy;
    if (segLenSq === 0) {
        return { t: 0, dist: Math.sqrt((pxk - axk) ** 2 + (pyk - ayk) ** 2) };
    }
    let t = ((pxk - axk) * dx + (pyk - ayk) * dy) / segLenSq;
    t = Math.max(0, Math.min(1, t));
    let projX = axk + t * dx;
    let projY = ayk + t * dy;
    let dist = Math.sqrt((pxk - projX) ** 2 + (pyk - projY) ** 2);
    return { t, dist };
}

class FollowSim {
    constructor(coordinates) {
        this.routeCoordinates = coordinates;
        this.lastCoordinateIndex = null;
    }

    findClosestCoordinate(longitude, latitude, coordinates) {
        let currentIndex = this.lastCoordinateIndex ?? 0;
        while (currentIndex < coordinates.length - 1) {
            let t = projectOnSegment(
                longitude, latitude,
                coordinates[currentIndex][0], coordinates[currentIndex][1],
                coordinates[currentIndex + 1][0], coordinates[currentIndex + 1][1]
            );
            let segLenKm = calculateDistance(
                coordinates[currentIndex][1], coordinates[currentIndex][0],
                coordinates[currentIndex + 1][1], coordinates[currentIndex + 1][0]
            );
            let distToStartKm = calculateDistance(
                latitude, longitude,
                coordinates[currentIndex][1], coordinates[currentIndex][0]
            );
            let distToEndKm = calculateDistance(
                latitude, longitude,
                coordinates[currentIndex + 1][1], coordinates[currentIndex + 1][0]
            );

            // Si on est très proche de la fin du segment, on passe au suivant
            if (distToEndKm < 0.015 && currentIndex < coordinates.length - 2) {
                currentIndex++;
                continue;
            }

            // Si on a dépassé le début du segment de plus de 15m, on passe au suivant
            if (distToStartKm > 0.015 && t.t === 0 && currentIndex < coordinates.length - 2) {
                currentIndex++;
                continue;
            }

            break;
        }
        this.lastCoordinateIndex = currentIndex;
        return currentIndex;
    }

    setBearing(coordinates, currentLat, currentLng, startIndex) {
        let hundredMeterAwayIndex = coordinates
            .slice(startIndex + 1)
            .findIndex(coord =>
                calculateDistance(currentLat, currentLng, coord[1], coord[0]) >= 0.05
            );
        hundredMeterAwayIndex = hundredMeterAwayIndex === -1
            ? coordinates.length - 1
            : hundredMeterAwayIndex + startIndex + 1;
        let bearing = calculateBearing(
            currentLng, currentLat,
            coordinates[hundredMeterAwayIndex][0], coordinates[hundredMeterAwayIndex][1]
        );
        return { bearing, hundredMeterAwayIndex };
    }
}

// Simulation des coordonnées (Le Caron -> Ruelle -> Galt)
// 1. Le Caron (vers l'Est)
// 2. Ruelle (virage à droite, vers le Sud)
// 3. Galt (virage à gauche, vers l'Est)
const coords = [
    [-73.5800, 45.5200], // Le Caron
    [-73.5790, 45.5200], // Début de la ruelle
    [-73.5790, 45.5195], // Fin de la ruelle / Début de Galt
    [-73.5780, 45.5195], // Galt
    [-73.5770, 45.5195], // Galt + ~50m
    [-73.5760, 45.5195]  // Galt + ~100m
];

const steps = [
    [-73.5795, 45.5200, "Sur Le Caron"],
    [-73.5790, 45.5200, "Début de la ruelle"],
    [-73.5790, 45.5198, "Dans la ruelle"],
    [-73.5790, 45.5195, "Arrivée sur Galt"],
    [-73.5785, 45.5195, "Sur Galt, ~25m"],
    [-73.5780, 45.5195, "Sur Galt, ~50m"],
    [-73.5775, 45.5195, "Sur Galt, ~75m"]
];

function runTest() {
    console.log("=== Test virage Le Caron -> Ruelle -> Galt ===");
    let sim = new FollowSim(coords);
    let failures = [];

    for (let i = 0; i < steps.length; i++) {
        let [lng, lat, desc] = steps[i];
        let idx = sim.findClosestCoordinate(lng, lat, sim.routeCoordinates);
        let bearingInfo = sim.setBearing(sim.routeCoordinates, lat, lng, idx);
        
        console.log(`Étape: ${desc}`);
        console.log(`  Position: [${lng}, ${lat}]`);
        console.log(`  Index le plus proche: ${idx}`);
        console.log(`  Index de direction: ${bearingInfo.hundredMeterAwayIndex}`);
        console.log(`  Direction (bearing): ${bearingInfo.bearing.toFixed(2)}°`);
        
        // La direction devrait rester vers l'Est (environ 90°) une fois sur Galt
        if (desc.includes("Sur Galt")) {
            if (bearingInfo.bearing < 45 || bearingInfo.bearing > 135) {
                failures.push(`Échec à l'étape "${desc}": la direction pointe vers l'arrière ou de travers (${bearingInfo.bearing.toFixed(2)}°)`);
            }
        }
    }

    if (failures.length > 0) {
        console.log("\nÉCHECS:");
        failures.forEach(f => console.log(" - " + f));
    } else {
        console.log("\nSUCCÈS: La direction est restée correcte sur Galt.");
    }
}

runTest();
