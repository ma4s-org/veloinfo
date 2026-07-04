// Test unitaire pour la logique de findClosestCoordinate et setBearing.
// On extrait les méthodes pures sans dépendance DOM.

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
    const a = Math.sin(Δφ / 2) ** 2 +
        Math.cos(φ1) * Math.cos(φ2) *
        Math.sin(Δλ / 2) ** 2;
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

// Simulation de FollowPanel avec seulement la logique de suivi
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

    setBearing(coordinates, currentLat, currentLng, startIndex) {
        let hundredMeterAwayIndex = coordinates
            .slice(startIndex + 1)
            .findIndex(coord =>
                calculateDistance(currentLat, currentLng, coord[1], coord[0]) >= 0.05
            );
        let targetIndex;
        let useSegmentBearing = false;
        if (hundredMeterAwayIndex === -1) {
            targetIndex = coordinates.length - 1;
            useSegmentBearing = true;
        } else {
            targetIndex = hundredMeterAwayIndex + startIndex + 1;
        }
        let bearing;
        if (useSegmentBearing) {
            let p1 = coordinates[Math.max(0, targetIndex - 1)];
            let p2 = coordinates[targetIndex];
            bearing = calculateBearing(p1[0], p1[1], p2[0], p2[1]);
        } else {
            bearing = calculateBearing(
                currentLng, currentLat,
                coordinates[targetIndex][0],
                coordinates[targetIndex][1]
            );
        }
        return { bearing, targetIndex, startIndex, useSegmentBearing };
    }
}

// --- Scénario de test ---
// Itinéraire : ligne droite vers le nord, puis virage à 90° vers l'est.
// Coordonnées en [lng, lat], approximativement 100m entre chaque point.
// ~0.001 deg lat ≈ 111m, ~0.001 deg lng ≈ 80m (à ~49° lat)

const coords = [
    [2.0, 49.0],   // 0: départ
    [2.0, 49.001], // 1: 111m au nord
    [2.0, 49.002], // 2: 222m au nord
    [2.0, 49.003], // 3: 333m au nord (virage ici)
    [2.001, 49.003], // 4: 80m à l'est
    [2.002, 49.003], // 5: 160m à l'est
    [2.003, 49.003], // 6: 240m à l'est
];

function runTest(testName, steps) {
    console.log(`\n=== ${testName} ===`);
    let sim = new FollowSim(coords);
    let failures = [];
    
    for (let i = 0; i < steps.length; i++) {
        let [lng, lat, desc, expectedBearing] = steps[i];
        let idx = sim.findClosestCoordinate(lng, lat, sim.routeCoordinates);
        let bearingInfo = sim.setBearing(sim.routeCoordinates, lat, lng, idx);
        
        // Stocke idx à une position fixe (4) peu importe la présence d'expectedBearing
        steps[i][4] = idx;
        let prevIdx = i > 0 ? steps[i - 1][4] : null;
        
        let idxOk = prevIdx === null || idx >= prevIdx;
        let bearingOk = bearingInfo.targetIndex >= idx;
        let expectedBearingOk = expectedBearing === undefined ||
            Math.abs(bearingInfo.bearing - expectedBearing) < 1;
        
        if (!idxOk) failures.push(`Step ${i}: index recule ${prevIdx}→${idx}`);
        if (!bearingOk) failures.push(`Step ${i}: bearing regarde en arrière (target ${bearingInfo.targetIndex} < current ${idx})`);
        if (!expectedBearingOk) failures.push(`Step ${i}: bearing=${bearingInfo.bearing.toFixed(1)}° attendu ~${expectedBearing}°`);
        
        let ok = idxOk && bearingOk && expectedBearingOk;
        console.log(`  Step ${i}: pos=(${lng},${lat}) idx=${idx} bearingTarget=${bearingInfo.targetIndex} bearing=${bearingInfo.bearing.toFixed(1)}° ${desc} ${ok ? '✅' : '❌'}`);
    }
    
    if (failures.length === 0) {
        console.log(`  → PASS`);
    } else {
        console.log(`  → FAIL:`);
        failures.forEach(f => console.log(`    ${f}`));
    }
    return failures.length === 0;
}

let allPass = true;

// Test 1: progression normale le long de l'itinéraire
allPass &= runTest('Progression normale', [
    [2.0, 49.0005, 'mi-chemin 0→1'],
    [2.0, 49.0015, 'mi-chemin 1→2'],
    [2.0, 49.0025, 'mi-chemin 2→3'],
    [2.0, 49.0030, 'au point 3 (virage)'],
    [2.0005, 49.003, 'mi-chemin 3→4'],
    [2.0015, 49.003, 'mi-chemin 4→5'],
    [2.0025, 49.003, 'mi-chemin 5→6'],
]);

// Test 2: on dépasse le point 3 mais il est encore le plus proche à vol d'oiseau
// (GPS placé légèrement à l'ouest du point 3, après avoir dépassé le virage)
allPass &= runTest('Dépassement du virage', [
    [2.0, 49.0025, 'avant virage, mi-chemin 2→3'],
    [2.0, 49.0031, 'juste après le point 3, légèrement au nord'],
    [2.0005, 49.003, 'sur le segment 3→4'],
    [2.0015, 49.003, 'sur le segment 4→5'],
]);

// Test 3: GPS placé exactement au point 3, puis avance vers l'est
allPass &= runTest('Au point de virage', [
    [2.0, 49.003, 'exactement au point 3'],
    [2.0001, 49.003, 'à peine après le virage'],
    [2.001, 49.003, 'au point 4'],
    [2.002, 49.003, 'au point 5'],
]);

// Test 4: GPS qui tremble autour du point de virage
allPass &= runTest('Jitter autour du virage', [
    [2.0, 49.0028, 'juste avant le point 3'],
    [2.0, 49.0032, 'juste après le point 3 (nord)'],
    [2.0, 49.0029, 'revient juste avant (jitter)'],
    [2.0003, 49.003, 'avance vers l\'est'],
    [2.0, 49.0031, 'jitter nord à nouveau'],
    [2.001, 49.003, 'avance vers l\'est'],
]);

// Test 5: fin d'itinéraire — on dépasse le dernier point de plus de 50m.
// Le bearing doit rester celui du dernier segment (vers l'est, 90°),
// et ne pas pointer vers l'arrière (vers le point 6 qu'on a dépassé).
allPass &= runTest('Fin d\'itinéraire dépassée', [
    [2.0025, 49.003, 'mi-chemin 5→6', 90],
    [2.003, 49.003, 'au point 6 (fin)', 90],
    [2.0035, 49.003, '50m après la fin', 90],
    [2.004, 49.003, '100m après la fin', 90],
]);

console.log(`\n${allPass ? '✅ Tous les tests passent' : '❌ Des tests échouent'}`);
process.exit(allPass ? 0 : 1);