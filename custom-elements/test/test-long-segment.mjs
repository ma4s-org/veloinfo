// Test : virage à 90°, point OSM suivant à >100m, GPS à <10m de l'intersection.
// C'est le scénario réel : après un virage, le prochain point OSM est loin,
// mais le cycliste est tout près de l'intersection.

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

    // setBearing avec le code actuel (slice à startIndex + 1)
    setBearingCurrent(coordinates, currentLat, currentLng, startIndex) {
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
// Rue A vers le nord, virage à 90° vers l'est au point 3 (intersection).
// Le point OSM 4 est à >100m à l'est (rue B longue sans point intermédiaire).
// Le cycliste passe l'intersection puis est à ~10m à l'est.

const metersToLat = 1 / 110574;
const metersToLng = 1 / (111320 * Math.cos(49 * Math.PI / 180));

const coords = [
    [2.0000, 49.0000], // 0: départ rue A
    [2.0000, 49.0003], // 1: ~33m nord
    [2.0000, 49.0006], // 2: ~66m nord
    [2.0000, 49.0009], // 3: ~100m nord — INTERSECTION (virage 90° vers l'est)
    [2.0015, 49.0009], // 4: ~105m à l'est — point OSM suivant à >100m !
    [2.0030, 49.0009], // 5: ~210m à l'est
    [2.0045, 49.0009], // 6: ~315m à l'est
];

let allPass = true;

function runTest(testName, steps) {
    console.log(`\n=== ${testName} ===`);
    let sim = new FollowSim(coords);
    let failures = [];

    for (let i = 0; i < steps.length; i++) {
        let [lng, lat, desc] = steps[i];
        let idx = sim.findClosestCoordinate(lng, lat, sim.routeCoordinates);
        let bearingInfo = sim.setBearingCurrent(sim.routeCoordinates, lat, lng, idx);

        let prevIdx = i > 0 ? steps[i - 1][3] : null;
        steps[i].push(idx);

        let idxOk = prevIdx === null || idx >= prevIdx;
        // Le bearing devrait pointer vers l'avant (target > idx)
        let bearingForward = bearingInfo.hundredMeterAwayIndex > idx;
        // Le bearing devrait pointer vers l'est (~90°) après le virage
        let b = bearingInfo.bearing;
        let bearingEast = b > 45 && b < 135;

        if (!idxOk) failures.push(`Step ${i}: index recule ${prevIdx}→${idx}`);
        if (!bearingForward) failures.push(`Step ${i}: bearingTarget (${bearingInfo.hundredMeterAwayIndex}) <= idx (${idx}), regarde le point courant ou en arrière`);
        if (!bearingEast && idx >= 3) failures.push(`Step ${i}: bearing ${b.toFixed(1)}° ne pointe pas vers l'est après le virage`);

        let bDir;
        if (b < 45 || b > 315) bDir = 'N';
        else if (b >= 45 && b < 135) bDir = 'E';
        else if (b >= 135 && b < 225) bDir = 'S';
        else bDir = 'W';

        let ok = idxOk && bearingForward && (idx < 3 || bearingEast);
        console.log(`  Step ${i}: pos=(${lng.toFixed(5)},${lat.toFixed(5)}) idx=${idx} target=${bearingInfo.hundredMeterAwayIndex} bearing=${b.toFixed(1)}° (${bDir}) ${desc} ${ok ? '✅' : '❌'}`);
    }

    if (failures.length === 0) {
        console.log(`  → PASS`);
    } else {
        console.log(`  → FAIL:`);
        failures.forEach(f => console.log(`    ${f}`));
    }
    return failures.length === 0;
}

// Test 1 : progression normale, virage, puis 5m, 8m, 10m après le virage
// Le point OSM suivant (index 4) est à ~105m de l'intersection.
allPass &= runTest('Virage 90° - point OSM suivant à >100m', [
    [2.0000, 49.0000, 'départ'],
    [2.0000, 49.0006, 'au point 2'],
    [2.0000, 49.0008, 'approche intersection'],
    [2.0000, 49.0009, 'au point 3 (intersection)'],
    [2.0000 + 3 * metersToLng, 49.0009, '3m après le virage'],
    [2.0000 + 5 * metersToLng, 49.0009, '5m après le virage'],
    [2.0000 + 8 * metersToLng, 49.0009, '8m après le virage'],
    [2.0000 + 10 * metersToLng, 49.0009, '10m après le virage'],
    [2.0000 + 15 * metersToLng, 49.0009, '15m après le virage'],
    [2.0000 + 20 * metersToLng, 49.0009, '20m après le virage'],
    [2.0000 + 50 * metersToLng, 49.0009, '50m après le virage'],
]);

// Test 2 : même chose mais le cycliste a un peu de jitter
allPass &= runTest('Virage 90° - jitter après le virage', [
    [2.0000, 49.0008, 'approche intersection'],
    [2.0000, 49.0009, 'au point 3 (intersection)'],
    [2.0000 + 5 * metersToLng, 49.0009 + 3 * metersToLat, '5m est + 3m nord'],
    [2.0000 + 8 * metersToLng, 49.0009 - 2 * metersToLat, '8m est - 2m sud'],
    [2.0000 + 10 * metersToLng, 49.0009 + 2 * metersToLat, '10m est + 2m nord'],
    [2.0000 + 10 * metersToLng, 49.0009 - 1 * metersToLat, '10m est - 1m sud'],
    [2.0000 + 20 * metersToLng, 49.0009, '20m après le virage'],
]);

// Test 3 : le point OSM suivant est ENCORE plus loin (>200m)
const coordsFar = [
    [2.0000, 49.0000], // 0
    [2.0000, 49.0006], // 1: ~66m nord
    [2.0000, 49.0009], // 2: ~100m nord — INTERSECTION
    [2.0030, 49.0009], // 3: ~210m à l'est — point OSM suivant à >200m !
    [2.0060, 49.0009], // 4: ~420m à l'est
];

function runFarTest(testName, steps) {
    console.log(`\n=== ${testName} ===`);
    let sim = new FollowSim(coordsFar);
    let failures = [];

    for (let i = 0; i < steps.length; i++) {
        let [lng, lat, desc] = steps[i];
        let idx = sim.findClosestCoordinate(lng, lat, sim.routeCoordinates);
        let bearingInfo = sim.setBearingCurrent(sim.routeCoordinates, lat, lng, idx);

        let prevIdx = i > 0 ? steps[i - 1][3] : null;
        steps[i].push(idx);

        let idxOk = prevIdx === null || idx >= prevIdx;
        let bearingForward = bearingInfo.hundredMeterAwayIndex > idx;
        let b = bearingInfo.bearing;
        let bearingEast = b > 45 && b < 135;

        if (!idxOk) failures.push(`Step ${i}: index recule ${prevIdx}→${idx}`);
        if (!bearingForward) failures.push(`Step ${i}: bearingTarget (${bearingInfo.hundredMeterAwayIndex}) <= idx (${idx}), regarde le point courant ou en arrière`);
        if (!bearingEast && idx >= 2) failures.push(`Step ${i}: bearing ${b.toFixed(1)}° ne pointe pas vers l'est après le virage`);

        let bDir;
        if (b < 45 || b > 315) bDir = 'N';
        else if (b >= 45 && b < 135) bDir = 'E';
        else if (b >= 135 && b < 225) bDir = 'S';
        else bDir = 'W';

        let ok = idxOk && bearingForward && (idx < 2 || bearingEast);
        console.log(`  Step ${i}: pos=(${lng.toFixed(5)},${lat.toFixed(5)}) idx=${idx} target=${bearingInfo.hundredMeterAwayIndex} bearing=${b.toFixed(1)}° (${bDir}) ${desc} ${ok ? '✅' : '❌'}`);
    }

    if (failures.length === 0) {
        console.log(`  → PASS`);
    } else {
        console.log(`  → FAIL:`);
        failures.forEach(f => console.log(`    ${f}`));
    }
    return failures.length === 0;
}

allPass &= runFarTest('Point OSM suivant à >200m', [
    [2.0000, 49.0000, 'départ'],
    [2.0000, 49.0006, 'au point 1'],
    [2.0000, 49.0009, 'au point 2 (intersection)'],
    [2.0000 + 3 * metersToLng, 49.0009, '3m après le virage'],
    [2.0000 + 5 * metersToLng, 49.0009, '5m après le virage'],
    [2.0000 + 8 * metersToLng, 49.0009, '8m après le virage'],
    [2.0000 + 10 * metersToLng, 49.0009, '10m après le virage'],
    [2.0000 + 15 * metersToLng, 49.0009, '15m après le virage'],
    [2.0000 + 50 * metersToLng, 49.0009, '50m après le virage'],
    [2.0000 + 100 * metersToLng, 49.0009, '100m après le virage'],
]);

console.log(`\n${allPass ? '✅ Tous les tests passent' : '❌ Des tests échouent'}`);
process.exit(allPass ? 0 : 1);