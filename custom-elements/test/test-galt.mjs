// Test : Le Caron → ruelle (droite) → Galt, la carte tourne vers l'arrière à ~50m sur Galt.
//
// Hypothèse : setBearing cherche un point à ≥50m en avant, ne le trouve pas
// (la route se termine bientôt ou les points suivants sont trop proches),
// et fallback sur coordinates.length - 1 qui est EN ARRIÈRE du cycliste.

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
    let abx = bxk - axk;
    let aby = byk - ayk;
    let abLenSq = abx * abx + aby * aby;
    if (abLenSq === 0) return 1;
    let apx = pxk - axk;
    let apy = pyk - ayk;
    let t = (apx * abx + apy * aby) / abLenSq;
    return t;
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
            let distPastStart = t * segLenKm * 1000;
            if (distPastStart > 15 || t > 0.5) {
                currentIndex++;
            } else {
                break;
            }
        }
        this.lastCoordinateIndex = currentIndex;
        return currentIndex;
    }

    // setBearing avec le code actuel (slice à startIndex + 1, fallback sur length-1)
    setBearing(coordinates, currentLat, currentLng, startIndex) {
        let hundredMeterAwayIndex = coordinates
            .slice(startIndex + 1)
            .findIndex(coord =>
                calculateDistance(currentLat, currentLng, coord[1], coord[0]) >= 0.05
            );
        hundredMeterAwayIndex = hundredMeterAwayIndex === -1
            ? coordinates.length - 1
            : hundredMeterAwayIndex + startIndex + 1;

        let targetCoord = coordinates[hundredMeterAwayIndex];
        let bearing = calculateBearing(
            currentLng, currentLat,
            targetCoord[0], targetCoord[1]
        );

        return { bearing, hundredMeterAwayIndex, targetCoord };
    }
}

// --- Scénario Le Caron → ruelle → Galt ---
// Le Caron va vers le nord-est. Ruelle part vers le sud-est (virage droite).
// Galt va vers le sud-est. La route fait un angle ~90° droite.
//
// Coordonnées approximatives dans Montréal (lat ~45.5x)
// On utilise des coords génériques avec les bonnes proportions.

const lat0 = 45.5200;
const lng0 = -73.5700;

// Conversion mètres → degrés à la latitude de Montréal
const mToLat = 1 / 110574;
const mToLng = 1 / (111320 * Math.cos(lat0 * Math.PI / 180));

// Le Caron va vers le NNE (bearing ~30°), ruelle vers l'ESE (bearing ~120°),
// Galt vers le SSE (bearing ~160°).
// Pour simplifier, on fait Le Caron vers le nord, ruelle vers l'est, Galt vers le sud-est.

// Le Caron (vers le nord, ~200m)
// Ruelle (vers l'est, ~80m) — courte ruelle
// Galt (vers le sud-est, ~200m) puis la route se termine

const coords = [
    // Le Caron (vers le nord)
    [lng0,              lat0,              ], // 0: début Le Caron
    [lng0,              lat0 + 50 * mToLat, ], // 1: 50m nord
    [lng0,              lat0 + 100 * mToLat,], // 2: 100m nord
    [lng0,              lat0 + 150 * mToLat,], // 3: 150m nord
    [lng0,              lat0 + 200 * mToLat,], // 4: intersection Le Caron / ruelle (virage droite)

    // Ruelle (vers l'est, courte ~80m)
    [lng0 + 40 * mToLng, lat0 + 200 * mToLat,], // 5: 40m dans la ruelle
    [lng0 + 80 * mToLng, lat0 + 200 * mToLat,], // 6: intersection ruelle / Galt (virage gauche)

    // Galt (vers le sud-est, ~200m)
    [lng0 + 80 * mToLng + 30 * mToLng, lat0 + 200 * mToLat - 30 * mToLat,], // 7: 42m sur Galt (SE, ~45°)
    [lng0 + 80 * mToLng + 60 * mToLng, lat0 + 200 * mToLat - 60 * mToLat,], // 8: 85m sur Galt
    [lng0 + 80 * mToLng + 90 * mToLng, lat0 + 200 * mToLat - 90 * mToLat,], // 9: 127m sur Galt
    [lng0 + 80 * mToLng + 120 * mToLng, lat0 + 200 * mToLat - 120 * mToLat,], // 10: 170m sur Galt
    [lng0 + 80 * mToLng + 150 * mToLng, lat0 + 200 * mToLat - 150 * mToLat,], // 11: 212m sur Galt — FIN DE ROUTE
];

let allPass = true;

function runTest(testName, steps) {
    console.log(`\n=== ${testName} ===`);
    let sim = new FollowSim(coords);
    let failures = [];

    for (let i = 0; i < steps.length; i++) {
        let [lng, lat, desc] = steps[i];
        let idx = sim.findClosestCoordinate(lng, lat, sim.routeCoordinates);
        let bearingInfo = sim.setBearing(sim.routeCoordinates, lat, lng, idx);

        steps[i].push(idx);

        let b = bearingInfo.bearing;
        let target = bearingInfo.hundredMeterAwayIndex;
        let targetCoord = bearingInfo.targetCoord;

        // Le target est-il en avant ou en arrière ?
        let targetDist = calculateDistance(lat, lng, targetCoord[1], targetCoord[0]) * 1000; // en mètres
        // On est à 50m sur Galt : le target devrait être devant nous (vers le SE)
        // Si le target est le dernier point et qu'on l'a dépassé, on regarde en arrière

        let bDir;
        if (b < 22.5 || b >= 337.5) bDir = 'N';
        else if (b >= 22.5 && b < 67.5) bDir = 'NE';
        else if (b >= 67.5 && b < 112.5) bDir = 'E';
        else if (b >= 112.5 && b < 157.5) bDir = 'SE';
        else if (b >= 157.5 && b < 202.5) bDir = 'S';
        else if (b >= 202.5 && b < 247.5) bDir = 'SO';
        else if (b >= 247.5 && b < 292.5) bDir = 'O';
        else bDir = 'NO';

        // Déterminer la direction attendue selon la position
        // Sur Le Caron (avant le virage ruelle) : vers le nord (~0°)
        // Dans la ruelle : vers l'est (~90°)
        // Sur Galt : vers le sud-est (~135°)
        let expectedDir = '?';
        let expectedBearingRange = null;
        if (idx <= 3) {
            expectedDir = 'N';
            expectedBearingRange = [-30, 30]; // ~0° ± 30
        } else if (idx <= 5) {
            expectedDir = 'NE/E';
            expectedBearingRange = [30, 120]; // transition
        } else if (idx <= 6) {
            expectedDir = 'E/SE';
            expectedBearingRange = [60, 150]; // transition
        } else {
            expectedDir = 'SE';
            expectedBearingRange = [105, 165]; // ~135° ± 30
        }

        // Normaliser le range
        let bearingOk = false;
        let bNorm = b;
        if (expectedBearingRange[0] < 0) {
            if (bNorm > 180) bNorm -= 360;
            bearingOk = bNorm >= expectedBearingRange[0] && bNorm <= expectedBearingRange[1];
        } else {
            bearingOk = bNorm >= expectedBearingRange[0] && bNorm <= expectedBearingRange[1];
        }

        // Vérifier si le target est derrière (bearing opposé à la direction de marche)
        let targetBehind = target <= idx;
        let looksBackward = false;
        if (idx >= 7) {
            // Sur Galt, on ne devrait jamais regarder vers le NO/N/O/SO
            if (b > 180 && b < 360) looksBackward = true;
        }

        if (!bearingOk) failures.push(`Step ${i}: bearing ${b.toFixed(1)}° (${bDir}) attendu ${expectedDir} (${expectedBearingRange[0]}°-${expectedBearingRange[1]}°)`);
        if (looksBackward) failures.push(`Step ${i}: bearing ${b.toFixed(1)}° (${bDir}) — la carte regarde VERS L'ARRIÈRE sur Galt !`);
        if (targetBehind) failures.push(`Step ${i}: target=${target} <= idx=${idx} — le point de visée est derrière`);

        console.log(`  Step ${i}: pos=(${lng.toFixed(5)},${lat.toFixed(5)}) idx=${idx} target=${target} dist=${targetDist.toFixed(0)}m bearing=${b.toFixed(1)}° (${bDir}) ${desc} ${bearingOk && !looksBackward ? '✅' : '❌'}`);
    }

    if (failures.length === 0) {
        console.log(`  → PASS`);
    } else {
        console.log(`  → FAIL:`);
        failures.forEach(f => console.log(`    ${f}`));
    }
    return failures.length === 0;
}

allPass &= runTest('Le Caron → ruelle → Galt (50m après Galt = bug)', [
    // Sur Le Caron
    [lng0, lat0, 'départ Le Caron'],
    [lng0, lat0 + 100 * mToLat, 'sur Le Caron 100m'],
    [lng0, lat0 + 180 * mToLat, 'sur Le Caron, proche intersection'],

    // Virage droite dans la ruelle
    [lng0 + 20 * mToLng, lat0 + 200 * mToLat, '20m dans la ruelle'],
    [lng0 + 60 * mToLng, lat0 + 200 * mToLat, '60m dans la ruelle'],

    // Virage sur Galt
    [lng0 + 80 * mToLng, lat0 + 200 * mToLat, 'au début de Galt'],
    [lng0 + 80 * mToLng + 10 * mToLng, lat0 + 200 * mToLat - 10 * mToLat, '14m sur Galt'],
    [lng0 + 80 * mToLng + 20 * mToLng, lat0 + 200 * mToLat - 20 * mToLat, '28m sur Galt'],
    [lng0 + 80 * mToLng + 30 * mToLng, lat0 + 200 * mToLat - 30 * mToLat, '42m sur Galt'],
    [lng0 + 80 * mToLng + 35 * mToLng, lat0 + 200 * mToLat - 35 * mToLat, '49m sur Galt ← BUG'],
    [lng0 + 80 * mToLng + 40 * mToLng, lat0 + 200 * mToLat - 40 * mToLat, '56m sur Galt'],
    [lng0 + 80 * mToLng + 50 * mToLng, lat0 + 200 * mToLat - 50 * mToLat, '70m sur Galt'],
    [lng0 + 80 * mToLng + 60 * mToLng, lat0 + 200 * mToLat - 60 * mToLat, '85m sur Galt'],
    [lng0 + 80 * mToLng + 90 * mToLng, lat0 + 200 * mToLat - 90 * mToLat, '127m sur Galt'],
]);

console.log(`\n${allPass ? '✅ Tous les tests passent' : '❌ Des tests échouent'}`);
process.exit(allPass ? 0 : 1);
