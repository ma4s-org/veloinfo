// Test : vérifier si findClosestCoordinate stagne au point d'intersection
// quand le segment suivant est très long (>100m) et le cycliste est à <10m.

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
            console.log(`    [findClosest] segment ${currentIndex}→${currentIndex+1}: t=${t.toFixed(4)} distPastStart=${distPastStart.toFixed(1)}m ${(distPastStart > 15 || t > 0.5) ? '→ avance' : '→ reste'}`);
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

const metersToLng = 1 / (111320 * Math.cos(49 * Math.PI / 180));

// Scénario : virage 90° au point 3, point suivant à 150m
const coords = [
    [2.0000, 49.0000], // 0
    [2.0000, 49.0003], // 1: ~33m nord
    [2.0000, 49.0006], // 2: ~66m nord
    [2.0000, 49.0009], // 3: ~100m nord — INTERSECTION
    [2.0020, 49.0009], // 4: ~140m à l'est — point OSM suivant lointain
    [2.0040, 49.0009], // 5: ~280m à l'est
];

let sim = new FollowSim(coords);

console.log('=== Progression vers le virage ===\n');

// Le cycliste approche l'intersection
let positions = [
    [2.0000, 49.0006, 'au point 2'],
    [2.0000, 49.0008, '30m avant intersection'],
    [2.0000, 49.0009, 'à l\'intersection'],
    [2.0000 + 5 * metersToLng, 49.0009, '5m après le virage'],
    [2.0000 + 10 * metersToLng, 49.0009, '10m après le virage'],
    [2.0000 + 50 * metersToLng, 49.0009, '50m après le virage'],
    [2.0000 + 100 * metersToLng, 49.0009, '100m après le virage'],
];

for (let [lng, lat, desc] of positions) {
    console.log(`\nPosition: (${lng.toFixed(5)}, ${lat.toFixed(5)}) — ${desc}`);
    let idx = sim.findClosestCoordinate(lng, lat, coords);
    console.log(`  → idx=${idx} (point: ${coords[idx]})`);
}