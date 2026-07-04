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

function distanceToSegment(px, py, ax, ay, bx, by) {
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

    if (abLenSq === 0) {
        let dx = pxk - axk;
        let dy = pyk - ayk;
        return Math.sqrt(dx * dx + dy * dy);
    }

    let apx = pxk - axk;
    let apy = pyk - ayk;
    let t = (apx * abx + apy * aby) / abLenSq;
    t = Math.max(0, Math.min(1, t));

    let projx = axk + t * abx;
    let projy = ayk + t * aby;

    let dx = pxk - projx;
    let dy = pyk - projy;
    return Math.sqrt(dx * dx + dy * dy);
}

function distanceToRoute(lng, lat, coords, closestIndex) {
    if (!coords || coords.length === 0) return Infinity;

    let minDistance = Infinity;
    let segments = [];
    if (closestIndex > 0) {
        segments.push([closestIndex - 1, closestIndex]);
    }
    if (closestIndex < coords.length - 1) {
        segments.push([closestIndex, closestIndex + 1]);
    }

    for (let [i, j] of segments) {
        let d = distanceToSegment(
            lng, lat,
            coords[i][0], coords[i][1],
            coords[j][0], coords[j][1]
        );
        if (d < minDistance) {
            minDistance = d;
        }
    }

    if (segments.length === 0) {
        return calculateDistance(lat, lng, coords[0][1], coords[0][0]);
    }

    return minDistance;
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
}

// Scénario : une route simple avec un virage à 90°
const coords = [
    [0, 0],         // Point 0
    [0, 0.001],     // Point 1 (100m au nord)
    [0.001, 0.001]  // Point 2 (100m à l'est)
];

let allPass = true;

function runTest(testName, steps) {
    console.log(`\n=== ${testName} ===`);
    let sim = new FollowSim(coords);
    let failures = [];

    for (let i = 0; i < steps.length; i++) {
        let [lng, lat, desc] = steps[i];
        let idx = sim.findClosestCoordinate(lng, lat, sim.routeCoordinates);
        let dist = distanceToRoute(lng, lat, sim.routeCoordinates, idx);

        // La distance devrait être en km. 0.05 km = 50m.
        let distMeters = dist * 1000;
        let isOnRoute = dist <= 0.05;

        console.log(`  Step ${i}: pos=(${lng.toFixed(5)},${lat.toFixed(5)}) idx=${idx} dist=${distMeters.toFixed(2)}m ${desc} ${isOnRoute ? '✅' : '❌'}`);

        if (!isOnRoute && desc.includes("sur la route")) {
            failures.push(`Step ${i}: dist=${distMeters.toFixed(2)}m — l'utilisateur est sur la route mais la distance est > 50m !`);
        }
    }

    if (failures.length === 0) {
        console.log(`  → PASS`);
    } else {
        console.log(`  → FAIL:`);
        failures.forEach(f => console.log(`    ${f}`));
    }
    return failures.length === 0;
}

allPass &= runTest('Distance sur l\'itinéraire', [
    [0, 0.0005, 'sur la route (milieu segment 0-1)'],
    [0, 0.0009, 'sur la route (proche du virage)'],
    [0, 0.001, 'sur la route (au virage)'],
    [0.0001, 0.001, 'sur la route (après le virage)'],
    [0.0005, 0.001, 'sur la route (milieu segment 1-2)'],
    [0.001, 0.001, 'sur la route (à la fin)'],
]);

console.log(`\n${allPass ? '✅ Tous les tests passent' : '❌ Des tests échouent'}`);
process.exit(allPass ? 0 : 1);
