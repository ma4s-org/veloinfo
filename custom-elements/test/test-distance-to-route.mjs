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

// Version alignée sur la prod : fenêtre étroite par défaut {back:2, forward:2},
// distance clampée au segment, retourne { distance, bestIndex }. Une fenêtre
// élargie {back:3, forward:10} est utilisée comme garde-fou avant recalcul
// pour mesurer la vraie distance à l'itinéraire et éviter les faux positifs
// quand le segment le plus proche est hors de la fenêtre étroite.
function distanceToRoute(lng, lat, coords, lastPassedVertexIndex, window = { back: 2, forward: 2 }) {
    if (!coords || coords.length === 0) return { distance: Infinity, bestIndex: null };

    let minDistance = Infinity;
    let bestIndex = null;
    let startSeg = Math.max(0, lastPassedVertexIndex - window.back);
    let endSeg = Math.min(coords.length - 2, lastPassedVertexIndex + window.forward);

    for (let i = startSeg; i <= endSeg; i++) {
        let d = distanceToSegment(
            lng, lat,
            coords[i][0], coords[i][1],
            coords[i + 1][0], coords[i + 1][1]
        );
        if (d < minDistance) {
            minDistance = d;
            bestIndex = i;
        }
    }

    if (minDistance === Infinity) {
        return { distance: calculateDistance(lat, lng, coords[0][1], coords[0][0]), bestIndex: null };
    }

    return { distance: minDistance, bestIndex };
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
        let { distance: dist } = distanceToRoute(lng, lat, sim.routeCoordinates, idx);

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

// Nouveau scénario : vertex en retard (lastPassedVertex pas encore avancé).
// L'utilisateur est physiquement entre les vertex 1 et 2, mais lastPassedVertex
// reste à 0 (bruit GPS ou segments courts). Avec la fenêtre étendue N=2, la
// distance au segment 1-2 doit être détectée et rester sous 50 m.
function runVertexLagTest() {
    console.log(`\n=== Vertex en retard (fenêtre étendue) ===`);
    let failures = [];

    // Position à 20 m à l'est du segment 1-2 (lat=0.001, lng=0.0002).
    // lastPassedVertex forcé à 0 (en retard).
    let { distance: dist, bestIndex } = distanceToRoute(0.0002, 0.001, coords, 0);
    let distMeters = dist * 1000;
    let isOnRoute = dist <= 0.05;
    console.log(`  pos=(0.00020,0.00100) lastIdx=0 dist=${distMeters.toFixed(2)}m bestSeg=${bestIndex} ${isOnRoute ? '✅' : '❌'}`);
    if (!isOnRoute) {
        failures.push(`Vertex lag: dist=${distMeters.toFixed(2)}m — devrait être sur la route (segment 1-2 détecté)`);
    }
    // bestIndex devrait pointer vers le segment 1-2 (index 1), permettant
    // l'auto-avance de lastPassedVertex.
    if (bestIndex !== 1) {
        failures.push(`Vertex lag: bestIndex=${bestIndex} attendu 1 (segment 1-2)`);
    }

    console.log(failures.length === 0 ? `  → PASS` : `  → FAIL:`);
    failures.forEach(f => console.log(`    ${f}`));
    return failures.length === 0;
}
allPass &= runVertexLagTest();

// Nouveau scénario : vraiment sorti de la route (>50 m à droite de la route,
// hors de tous les segments voisins). Le recalcul doit être justifié.
function runReallyOffRouteTest() {
    console.log(`\n=== Vraiment sorti de l'itinéraire ===`);
    let failures = [];

    // Position à ~80 m au nord du segment 1-2 (lat=0.00172, lng=0.0005).
    // Le segment 1-2 est à lat=0.001 (horizontal). 0.00072 degré de latitude
    // ≈ 80 m. Aucun segment voisin ne passe par là.
    let { distance: dist } = distanceToRoute(0.0005, 0.00172, coords, 1);
    let distMeters = dist * 1000;
    let isOffRoute = dist > 0.05;
    console.log(`  pos=(0.00050,0.00172) lastIdx=1 dist=${distMeters.toFixed(2)}m ${isOffRoute ? '✅' : '❌'}`);
    if (!isOffRoute) {
        failures.push(`Really off-route: dist=${distMeters.toFixed(2)}m — devrait être > 50m (recalcul justifié)`);
    }

    console.log(failures.length === 0 ? `  → PASS` : `  → FAIL:`);
    failures.forEach(f => console.log(`    ${f}`));
    return failures.length === 0;
}
allPass &= runReallyOffRouteTest();

// Nouveau scénario : segment proche hors fenêtre étroite mais dans fenêtre
// élargie. Reproduit le bug de recalcul injustifié : l'utilisateur est à
// ~30 m d'un segment situé 6 vertices en avant de lastPassedVertexIndex.
// La fenêtre étroite {back:2, forward:2} ne le voit pas et retourne > 50 m
// (faux positif de recalcul). La fenêtre élargie {back:3, forward:10} doit
// le détecter, retourner <= 50 m, et bestIndex doit pointer sur le bon
// segment pour l'auto-avance.
function runWideWindowGuardTest() {
    console.log(`\n=== Segment proche hors fenêtre étroite (garde-fou élargi) ===`);
    let failures = [];

    // Route en épingle : on remonte vers le nord avec des segments courts de
    // ~20 m, puis on repart vers l'est. lastPassedVertexIndex reste à 0
    // (en retard). Le segment 6-7 (horizontal, ~6 segments en avant) passe
    // à ~30 m au sud de la position — c'est le vrai segment le plus proche.
    let epingle = [
        [0.00000, 0.00000], // 0
        [0.00000, 0.00020], // 1  ~22 m au nord
        [0.00000, 0.00040], // 2  ~44 m
        [0.00000, 0.00060], // 3  ~67 m
        [0.00000, 0.00080], // 4  ~89 m
        [0.00000, 0.00100], // 5  ~111 m
        [0.00030, 0.00100], // 6  segment 6-7 horizontal vers l'est
        [0.00060, 0.00100], // 7
    ];

    // Position à ~30 m au nord du segment 6-7 (lat = 0.00100 + 0.00027 ≈ +30 m,
    // lng au milieu du segment 6-7 = 0.00045).
    let posLng = 0.00045;
    let posLat = 0.00127;

    // Fenêtre étroite (lastPassedVertexIndex = 0) : ne voit que les segments
    // 0-1 à 1-2, verticaux à lng=0. La position est à ~100 m à l'est → > 50 m.
    let narrow = distanceToRoute(posLng, posLat, epingle, 0, { back: 2, forward: 2 });
    let narrowM = narrow.distance * 1000;
    let narrowTriggersRecalc = narrow.distance > 0.05;
    console.log(`  étroit: pos=(${posLng.toFixed(5)},${posLat.toFixed(5)}) lastIdx=0 dist=${narrowM.toFixed(2)}m bestSeg=${narrow.bestIndex} → recalcul=${narrowTriggersRecalc ? 'OUI (faux positif)' : 'non'}`);
    if (!narrowTriggersRecalc) {
        failures.push(`Fenêtre étroite devrait déclencher > 50 m (pour reproduire le bug), a obtenu ${narrowM.toFixed(2)}m`);
    }

    // Garde-fou élargi : doit voir le segment 6-7 à ~30 m.
    let wide = distanceToRoute(posLng, posLat, epingle, 0, { back: 3, forward: 10 });
    let wideM = wide.distance * 1000;
    let wideOnRoute = wide.distance <= 0.05;
    console.log(`  élargi: dist=${wideM.toFixed(2)}m bestSeg=${wide.bestIndex} → sur route=${wideOnRoute ? 'OUI' : 'non'}`);
    if (!wideOnRoute) {
        failures.push(`Fenêtre élargie devrait trouver le segment 6-7 à < 50 m, a obtenu ${wideM.toFixed(2)}m`);
    }
    if (wide.bestIndex !== 6) {
        failures.push(`bestIndex devrait pointer sur le segment 6-7 (index 6), a obtenu ${wide.bestIndex}`);
    }

    // Vérifie qu'un vrai hors-route reste détecté même avec la fenêtre élargie.
    // Position à ~120 m à l'est de l'épingle, hors de tous les segments.
    let offLng = 0.00120;
    let offLat = 0.00100;
    let offWide = distanceToRoute(offLng, offLat, epingle, 0, { back: 3, forward: 10 });
    let offWideM = offWide.distance * 1000;
    let offTriggers = offWide.distance > 0.05;
    console.log(`  élargi (vraiment hors route): dist=${offWideM.toFixed(2)}m → recalcul=${offTriggers ? 'OUI' : 'non'}`);
    if (!offTriggers) {
        failures.push(`Vrai hors-route devrait rester > 50 m avec fenêtre élargie, a obtenu ${offWideM.toFixed(2)}m`);
    }

    console.log(failures.length === 0 ? `  → PASS` : `  → FAIL:`);
    failures.forEach(f => console.log(`    ${f}`));
    return failures.length === 0;
}
allPass &= runWideWindowGuardTest();

console.log(`\n${allPass ? '✅ Tous les tests passent' : '❌ Des tests échouent'}`);
process.exit(allPass ? 0 : 1);