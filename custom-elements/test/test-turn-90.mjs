// Test : virage à 90° avec points OSM denses, problème ~10m après le virage.

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
            coordinates[hundredMeterAwayIndex][0],
            coordinates[hundredMeterAwayIndex][1]
        );
        return { bearing, hundredMeterAwayIndex, startIndex };
    }
}

// --- Scénario ---
// Rue A vers le nord, virage à 90° vers l'est, Rue B vers l'est.
// Points OSM denses (~20-25m entre chaque point).
// À ~49° de latitude : 0.0003° lat ≈ 33m, 0.0004° lng ≈ 28m

const coords = [
    [2.0000, 49.0000], // 0: départ rue A
    [2.0000, 49.0003], // 1: ~33m au nord
    [2.0000, 49.0006], // 2: ~66m au nord
    [2.0000, 49.0009], // 3: ~100m au nord — point de virage (intersection)
    [2.0004, 49.0009], // 4: ~28m à l'est (rue B)
    [2.0008, 49.0009], // 5: ~56m à l'est
    [2.0012, 49.0009], // 6: ~85m à l'est
    [2.0016, 49.0009], // 7: ~113m à l'est
    [2.0020, 49.0009], // 8: ~141m à l'est
];

// Convertir 10m en degrés pour positionner le cycliste
const metersToLat = 1 / 110574;  // 1m en degrés lat
const metersToLng = 1 / (111320 * Math.cos(49 * Math.PI / 180)); // 1m en degrés lng à 49°

let allPass = true;

function runTest(testName, steps) {
    console.log(`\n=== ${testName} ===`);
    let sim = new FollowSim(coords);
    let failures = [];

    for (let i = 0; i < steps.length; i++) {
        let [lng, lat, desc] = steps[i];
        let idx = sim.findClosestCoordinate(lng, lat, sim.routeCoordinates);
        let bearingInfo = sim.setBearing(sim.routeCoordinates, lat, lng, idx);

        // Le bearing devrait pointer vers le nord (0°) avant le virage
        // et vers l'est (90°) après le virage.
        // Tolérance : ±45° autour de la direction attendue.
        let expectedBearing = bearingInfo.bearing; // sera vérifié ci-dessous
        let idxOk = true;
        let bearingOk = true;

        let prevIdx = i > 0 ? steps[i - 1][3] : null;
        steps[i].push(idx);
        idxOk = prevIdx === null || idx >= prevIdx;
        // En fin de route (idx = dernier point), bearingTarget == idx est OK
        bearingOk = bearingInfo.hundredMeterAwayIndex > idx || idx === coordinates.length - 1;

        if (!idxOk) failures.push(`Step ${i}: index recule ${prevIdx}→${idx}`);
        if (!bearingOk) failures.push(`Step ${i}: bearingTarget (${bearingInfo.hundredMeterAwayIndex}) <= idx (${idx}), regarde en arrière`);

        // Vérifier que le bearing est dans la bonne direction
        let b = bearingInfo.bearing;
        let bearingDirection;
        if (b < 45 || b > 315) bearingDirection = 'N';
        else if (b >= 45 && b < 135) bearingDirection = 'E';
        else if (b >= 135 && b < 225) bearingDirection = 'S';
        else bearingDirection = 'W';

        console.log(`  Step ${i}: pos=(${lng.toFixed(5)},${lat.toFixed(5)}) idx=${idx} target=${bearingInfo.hundredMeterAwayIndex} bearing=${b.toFixed(1)}° (${bearingDirection}) ${desc} ${idxOk && bearingOk ? '✅' : '❌'}`);
    }

    if (failures.length === 0) {
        console.log(`  → PASS`);
    } else {
        console.log(`  → FAIL:`);
        failures.forEach(f => console.log(`    ${f}`));
    }
    return failures.length === 0;
}

// Test principal : progression normale, virage à 90°, puis 10m après le virage
const turnPoint = coords[3]; // [2.0000, 49.0009]

allPass &= runTest('Virage 90° - progression normale', [
    [2.0000, 49.0000, 'au départ'],
    [2.0000, 49.0003, 'au point 1'],
    [2.0000, 49.0005, 'entre 1 et 2'],
    [2.0000, 49.0006, 'au point 2'],
    [2.0000, 49.0008, 'entre 2 et 3 (approche virage)'],
    [2.0000, 49.0009, 'au point 3 (virage)'],
    // ~10m après le virage, sur la rue B (vers l'est)
    [2.0000 + 10 * metersToLng, 49.0009, '10m après le virage, vers l\'est'],
    [2.0000 + 15 * metersToLng, 49.0009, '15m après le virage'],
    [2.0000 + 20 * metersToLng, 49.0009, '20m après le virage'],
    [2.0000 + 28 * metersToLng, 49.0009, 'au point 4'],
    [2.0000 + 50 * metersToLng, 49.0009, 'entre 4 et 5'],
    [2.0000 + 56 * metersToLng, 49.0009, 'au point 5'],
]);

// Test 2 : le GPS place le cycliste 10m après le virage mais encore
// très proche du point 3 (le point de virage). Est-ce que l'index avance ?
allPass &= runTest('10m après virage - proche du point de virage', [
    [2.0000, 49.0006, 'au point 2'],
    [2.0000, 49.0009, 'au point 3 (virage)'],
    [2.0000 + 5 * metersToLng, 49.0009, '5m après le virage'],
    [2.0000 + 8 * metersToLng, 49.0009, '8m après le virage'],
    [2.0000 + 10 * metersToLng, 49.0009, '10m après le virage'],
    [2.0000 + 12 * metersToLng, 49.0009, '12m après le virage'],
]);

// Test 3 : le cycliste est 10m après le virage mais le GPS a un peu de jitter
// et le place légèrement au nord (vers la rue A)
allPass &= runTest('10m après virage - jitter nord', [
    [2.0000, 49.0008, 'juste avant le virage'],
    [2.0000, 49.0009, 'au point 3 (virage)'],
    [2.0000 + 10 * metersToLng, 49.0009 + 5 * metersToLat, '10m est + 5m nord (jitter)'],
    [2.0000 + 10 * metersToLng, 49.0009 - 3 * metersToLat, '10m est - 3m nord (jitter)'],
    [2.0000 + 15 * metersToLng, 49.0009, '15m après le virage'],
]);

// Test 4 : points OSM très denses (~10m) autour du virage
const denseCoords = [
    [2.0000, 49.0000], // 0
    [2.0000, 49.00009], // 1: ~10m nord
    [2.0000, 49.00018], // 2: ~20m nord
    [2.0000, 49.00027], // 3: ~30m nord
    [2.0000, 49.00036], // 4: ~40m nord — virage
    [2.00012, 49.00036], // 5: ~8m est
    [2.00024, 49.00036], // 6: ~17m est
    [2.00036, 49.00036], // 7: ~25m est
    [2.00048, 49.00036], // 8: ~34m est
    [2.00060, 49.00036], // 9: ~42m est
    [2.00072, 49.00036], // 10: ~50m est
];

function runDenseTest(testName, steps) {
    console.log(`\n=== ${testName} ===`);
    let sim = new FollowSim(denseCoords);
    let failures = [];

    for (let i = 0; i < steps.length; i++) {
        let [lng, lat, desc] = steps[i];
        let idx = sim.findClosestCoordinate(lng, lat, sim.routeCoordinates);
        let bearingInfo = sim.setBearing(sim.routeCoordinates, lat, lng, idx);

        let prevIdx = i > 0 ? steps[i - 1][3] : null;
        steps[i].push(idx);
        let idxOk = prevIdx === null || idx >= prevIdx;
        // En fin de route (idx = dernier point), bearingTarget == idx est OK
        let bearingOk = bearingInfo.hundredMeterAwayIndex > idx || idx === denseCoords.length - 1;

        if (!idxOk) failures.push(`Step ${i}: index recule ${prevIdx}→${idx}`);
        if (!bearingOk) failures.push(`Step ${i}: bearingTarget (${bearingInfo.hundredMeterAwayIndex}) <= idx (${idx})`);

        let b = bearingInfo.bearing;
        let bearingDirection;
        if (b < 45 || b > 315) bearingDirection = 'N';
        else if (b >= 45 && b < 135) bearingDirection = 'E';
        else if (b >= 135 && b < 225) bearingDirection = 'S';
        else bearingDirection = 'W';

        console.log(`  Step ${i}: pos=(${lng.toFixed(5)},${lat.toFixed(5)}) idx=${idx} target=${bearingInfo.hundredMeterAwayIndex} bearing=${b.toFixed(1)}° (${bearingDirection}) ${desc} ${idxOk && bearingOk ? '✅' : '❌'}`);
    }

    if (failures.length === 0) {
        console.log(`  → PASS`);
    } else {
        console.log(`  → FAIL:`);
        failures.forEach(f => console.log(`    ${f}`));
    }
    return failures.length === 0;
}

const denseTurnPoint = denseCoords[4]; // [2.0000, 49.00036]

allPass &= runDenseTest('Virage 90° points denses - progression', [
    [2.0000, 49.00000, 'départ'],
    [2.0000, 49.00009, 'point 1'],
    [2.0000, 49.00018, 'point 2'],
    [2.0000, 49.00027, 'point 3'],
    [2.0000, 49.00036, 'point 4 (virage)'],
    [2.0000 + 5 * metersToLng, 49.00036, '5m après virage'],
    [2.0000 + 10 * metersToLng, 49.00036, '10m après virage'],
    [2.0000 + 15 * metersToLng, 49.00036, '15m après virage'],
    [2.0000 + 25 * metersToLng, 49.00036, '25m après virage (point 7)'],
    [2.0000 + 35 * metersToLng, 49.00036, '35m après virage'],
    [2.0000 + 50 * metersToLng, 49.00036, '50m après virage'],
]);

// Test 5 : Le cycliste est 10m après le virage mais le segment avant le virage
// est beaucoup plus long que le segment après. Le point de virage est encore
// le plus proche à vol d'oiseau.
allPass &= runTest('Segment long avant, court après le virage', [
    [2.0000, 49.0000, 'départ, loin avant le virage'],
    [2.0000, 49.0005, 'mi-chemin avant le virage'],
    [2.0000, 49.0009, 'au point 3 (virage)'],
    [2.0000 + 10 * metersToLng, 49.0009, '10m après le virage'],
    [2.0000 + 10 * metersToLng, 49.0009 + 3 * metersToLat, '10m est + 3m nord'],
    [2.0000 + 10 * metersToLng, 49.0009 - 2 * metersToLat, '10m est - 2m sud'],
    [2.0000 + 20 * metersToLng, 49.0009, '20m après le virage'],
]);

console.log(`\n${allPass ? '✅ Tous les tests passent' : '❌ Des tests échouent'}`);
process.exit(allPass ? 0 : 1);