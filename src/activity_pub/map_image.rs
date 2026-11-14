//! Génération d'une vignette cartographique pour les messages ActivityPub.
//!
//! L'image est volontairement simple : fond clair, réseau de rues issu de la
//! table `edge`, tracé de la contribution et repère central. Elle est ensuite
//! jointe à la Note comme image, ce qui permet à Mastodon d'afficher un aperçu
//! de la carte dans le message.

use anyhow::{anyhow, bail, Context, Result};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat;
use chromiumoxide::handler::viewport::Viewport;
use chromiumoxide::page::ScreenshotParams;
use futures::StreamExt;
use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use serde_json::Value;
use sqlx::PgPool;
use std::io::Cursor;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::Mutex;

/// Un seul Chromium à la fois : les captures sont mises en cache, et cela
/// évite de lancer plusieurs navigateurs en parallèle sur le serveur.
static HEADLESS_LOCK: Mutex<()> = Mutex::const_new(());

const WIDTH: u32 = 1200;
const HEIGHT: u32 = 630;
const PADDING: f64 = 48.0;
const MAX_EDGES: i64 = 2500;

/// Largeur minimale de la zone représentée par la vignette, en mètres.
/// Si la contribution est plus grande, la vue s'élargit pour la contenir.
const VIEW_WIDTH_METERS: f64 = 400.0;

const BACKGROUND: Rgba<u8> = Rgba([248, 250, 252, 255]);
const ROAD: Rgba<u8> = Rgba([203, 213, 225, 255]);
const BIKE_ROUTE: Rgba<u8> = Rgba([14, 165, 233, 255]);
const REPORT_FILL: Rgba<u8> = Rgba([37, 99, 235, 55]);
const REPORT_LINE: Rgba<u8> = Rgba([37, 99, 235, 255]);
const MARKER: Rgba<u8> = Rgba([37, 99, 235, 255]);
const MARKER_INNER: Rgba<u8> = Rgba([255, 255, 255, 255]);

#[derive(Debug, Default)]
struct ParsedShape {
    lines: Vec<Vec<[f64; 2]>>,
    polygons: Vec<Vec<Vec<[f64; 2]>>>,
}

impl ParsedShape {
    fn bounds(&self) -> Option<(f64, f64, f64, f64)> {
        let mut bounds: Option<(f64, f64, f64, f64)> = None;
        for point in self
            .lines
            .iter()
            .flatten()
            .chain(self.polygons.iter().flatten().flatten())
        {
            let (lng, lat) = (point[0], point[1]);
            bounds = Some(match bounds {
                Some((min_x, min_y, max_x, max_y)) => (
                    min_x.min(lng),
                    min_y.min(lat),
                    max_x.max(lng),
                    max_y.max(lat),
                ),
                None => (lng, lat, lng, lat),
            });
        }
        bounds
    }

    fn center(&self) -> Option<(f64, f64)> {
        let mut sum = (0.0, 0.0);
        let mut count = 0usize;
        for point in self
            .lines
            .iter()
            .flatten()
            .chain(self.polygons.iter().flatten().flatten())
        {
            sum.0 += point[0];
            sum.1 += point[1];
            count += 1;
        }
        (count > 0).then_some((sum.0 / count as f64, sum.1 / count as f64))
    }
}

/// Génère la vignette d'une contribution à partir de sa géométrie stockée
/// dans PostGIS.
pub async fn render_report_map(conn: &PgPool, report_id: i32) -> Result<Vec<u8>> {
    let report_json: String = sqlx::query_scalar(
        "SELECT ST_AsGeoJSON(geom) FROM report WHERE id = $1",
    )
    .bind(report_id)
    .fetch_optional(conn)
    .await?
    .ok_or_else(|| anyhow!("contribution introuvable"))?;

    let report = parse_shape(&report_json)?;
    let (min_x, min_y, max_x, max_y) = report
        .bounds()
        .ok_or_else(|| anyhow!("géométrie vide"))?;
    let (min_x, min_y, max_x, max_y) = view_bounds(min_x, min_y, max_x, max_y);

    let edges: Vec<(String, bool)> = sqlx::query_as(
        r#"SELECT ST_AsGeoJSON(e.geom), e.in_bicycle_route
           FROM edge e
           WHERE e.geom && ST_MakeEnvelope($1, $2, $3, $4, 3857)
           ORDER BY e.in_bicycle_route
           LIMIT $5"#,
    )
    .bind(min_x)
    .bind(min_y)
    .bind(max_x)
    .bind(max_y)
    .bind(MAX_EDGES)
    .fetch_all(conn)
    .await
    .unwrap_or_default();

    tokio::task::spawn_blocking(move || {
        render_png(&report, &edges, min_x, min_y, max_x, max_y)
    })
    .await?
}

/// Récupère l'image depuis le cache disque, ou la génère avec Chromium
/// headless. En cas d'échec de Chromium, on retombe sur le rendu Rust.
pub async fn render_report_map_cached(
    conn: &PgPool,
    report_id: i32,
) -> Result<Vec<u8>> {
    let exists: Option<i32> =
        sqlx::query_scalar("SELECT 1 FROM report WHERE id = $1")
            .bind(report_id)
            .fetch_optional(conn)
            .await?;
    if exists.is_none() {
        bail!("contribution introuvable");
    }

    let cache_path = cache_path(report_id);
    if let Ok(png) = tokio::fs::read(&cache_path).await {
        if png.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Ok(png);
        }
    }

    let png = match render_report_map_headless(report_id).await {
        Ok(png) => png,
        Err(error) => {
            eprintln!(
                "ActivityPub: rendu headless de la carte {} impossible, repli sur le rendu Rust: {:#}",
                report_id, error
            );
            render_report_map(conn, report_id).await?
        }
    };

    if let Some(parent) = cache_path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let _ = tokio::fs::write(&cache_path, &png).await;
    Ok(png)
}

fn cache_path(report_id: i32) -> PathBuf {
    let directory = std::env::var("MAP_CACHE_DIR").unwrap_or_else(|_| {
        std::env::temp_dir()
            .join("veloinfo-maps")
            .to_string_lossy()
            .into_owned()
    });
    PathBuf::from(directory).join(format!("report-{}-400m-v1.png", report_id))
}

/// Capture la page MapLibre avec Chromium en mode headless, via le protocole
/// DevTools. On attend explicitement le signal `window.__mapReady` avant de
/// prendre la capture, ce qui évite les images à moitié rendues.
async fn render_report_map_headless(report_id: i32) -> Result<Vec<u8>> {
    let _guard = HEADLESS_LOCK.lock().await;
    let internal_base = std::env::var("FEDIVERSE_RENDER_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:3000".to_string())
        .trim_end_matches('/')
        .to_string();
    let url = format!("{}/report/{}/map-render", internal_base, report_id);

    let mut builder = BrowserConfig::builder()
        .no_sandbox()
        .new_headless_mode()
        .window_size(WIDTH, HEIGHT)
        .viewport(Viewport {
            width: WIDTH,
            height: HEIGHT,
            device_scale_factor: Some(1.0),
            emulating_mobile: false,
            is_landscape: false,
            has_touch: false,
        })
        .args([
            "--disable-dev-shm-usage",
            "--hide-scrollbars",
            "--use-angle=swiftshader",
            "--enable-unsafe-swiftshader",
            "--run-all-compositor-stages-before-draw",
        ]);
    if let Ok(chromium) = std::env::var("CHROMIUM_BIN") {
        builder = builder.chrome_executable(chromium);
    }
    let config = builder.build().map_err(|error| anyhow!(error))?;

    let (mut browser, mut handler) = Browser::launch(config)
        .await
        .context("lancement de Chromium headless")?;
    let handler_task = tokio::spawn(async move {
        while let Some(event) = handler.next().await {
            if event.is_err() {
                break;
            }
        }
    });

    let result = async {
        let page = browser
            .new_page(&url)
            .await
            .context("ouverture de la page de rendu")?;

        let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
        loop {
            if let Ok(value) = page.evaluate("window.__mapReady === true").await {
                if value.into_value::<bool>().unwrap_or(false) {
                    break;
                }
            }
            if tokio::time::Instant::now() >= deadline {
                bail!("la carte n'a pas fini de se charger dans Chromium");
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }

        // Laisse le compositeur peindre la dernière frame.
        tokio::time::sleep(Duration::from_millis(500)).await;

        let png = page
            .screenshot(
                ScreenshotParams::builder()
                    .format(CaptureScreenshotFormat::Png)
                    .build(),
            )
            .await
            .context("capture PNG de la carte")?;
        if !png.starts_with(b"\x89PNG\r\n\x1a\n") {
            bail!("Chromium n'a pas produit un PNG");
        }
        Ok::<Vec<u8>, anyhow::Error>(png)
    }
    .await;

    let _ = browser.close().await;
    handler_task.abort();
    result
}

/// Page HTML minimale chargée par Chromium pour capturer la carte.
pub async fn report_map_html(
    conn: &PgPool,
    report_id: i32,
    public_base: &str,
) -> Result<String> {
    let geometry_json: String = sqlx::query_scalar(
        "SELECT ST_AsGeoJSON(ST_Transform(geom, 4326)) FROM report WHERE id = $1",
    )
    .bind(report_id)
    .fetch_optional(conn)
    .await?
    .ok_or_else(|| anyhow!("contribution introuvable"))?;

    let focus = format!(
        r#"{{"type":"Feature","properties":{{}},"geometry":{}}}"#,
        geometry_json
    );
    let public_base_json = serde_json::to_string(public_base)?;

    Ok(MAP_HTML
        .replace("__FOCUS__", &focus)
        .replace("__PUBLIC_BASE__", &public_base_json))
}

const MAP_HTML: &str = r#"<!doctype html>
<html lang="fr">
<head>
<meta charset="utf-8">
<title>Vélo info – carte</title>
<link href="/node_modules/maplibre-gl/dist/maplibre-gl.css" rel="stylesheet" />
<style>
html, body, #map {
  margin: 0;
  padding: 0;
  width: 100%;
  height: 100%;
  overflow: hidden;
  background: #f8fafc;
}
.maplibregl-ctrl-attrib { display: none !important; }
</style>
</head>
<body>
<div id="map"></div>
<script type="module">
import * as maplibregl from '/node_modules/maplibre-gl/dist/maplibre-gl.mjs';

const focus = __FOCUS__;
const publicBase = __PUBLIC_BASE__;
const VIEW_WIDTH = 400;
const VIEW_ASPECT = 1200 / 630;

function collect(coords, box) {
  if (typeof coords[0] === 'number') {
    box[0] = Math.min(box[0], coords[0]);
    box[1] = Math.min(box[1], coords[1]);
    box[2] = Math.max(box[2], coords[0]);
    box[3] = Math.max(box[3], coords[1]);
  } else {
    for (const child of coords) collect(child, box);
  }
}

function viewBounds(geometry) {
  const box = [Infinity, Infinity, -Infinity, -Infinity];
  collect(geometry.coordinates, box);
  const center = [(box[0] + box[2]) / 2, (box[1] + box[3]) / 2];
  const metersPerDegreeLat = 110540;
  const metersPerDegreeLng = 111320 * Math.cos(center[1] * Math.PI / 180);
  const reportWidth = (box[2] - box[0]) * metersPerDegreeLng;
  const reportHeight = (box[3] - box[1]) * metersPerDegreeLat;

  let width = Math.max(VIEW_WIDTH, reportWidth + 20);
  let height = width / VIEW_ASPECT;
  if (reportHeight + 20 > height) {
    height = reportHeight + 20;
    width = height * VIEW_ASPECT;
  }

  const lngDelta = width / metersPerDegreeLng;
  const latDelta = height / metersPerDegreeLat;
  return [
    [center[0] - lngDelta / 2, center[1] - latDelta / 2],
    [center[0] + lngDelta / 2, center[1] + latDelta / 2]
  ];
}

async function main() {
  const response = await fetch('/style.json', { cache: 'no-store' });
  const style = await response.json();
  const internalBase = window.location.origin;
  const rewritten = JSON.parse(
    JSON.stringify(style).split(publicBase).join(internalBase)
  );

  const map = new maplibregl.Map({
    container: 'map',
    style: rewritten,
    center: [0, 0],
    zoom: 2,
    interactive: false,
    attributionControl: false,
    preserveDrawingBuffer: true,
    fadeDuration: 0,
    canvasContextAttributes: { antialias: true }
  });

  let mapLoaded = false;
  const ready = () => {
    window.__mapReady = true;
    document.title = 'ready';
  };

  map.on('error', (event) => console.error('Map error', event.error));
  map.on('load', () => {
    mapLoaded = true;
    map.addSource('veloinfo-focus', { type: 'geojson', data: focus });
    map.addLayer({
      id: 'veloinfo-focus-fill',
      type: 'fill',
      source: 'veloinfo-focus',
      paint: { 'fill-color': '#2563eb', 'fill-opacity': 0.22 }
    });
    map.addLayer({
      id: 'veloinfo-focus-line',
      type: 'line',
      source: 'veloinfo-focus',
      paint: { 'line-color': '#2563eb', 'line-width': 5 }
    });
    map.fitBounds(viewBounds(focus.geometry), {
      padding: 0,
      duration: 0,
      maxZoom: 22
    });
    // Laisse le compositeur peindre les dernières tuiles avant la capture.
    map.once('idle', () => setTimeout(ready, 1200));
  });
  // Si la carte a commencé à se charger mais que `idle` tarde, on capture
  // quand même après 15 s. Si le style n'a jamais chargé, on laisse le
  // timeout côté Rust déclencher le rendu de repli.
  setTimeout(() => {
    if (mapLoaded) ready();
  }, 15000);
}

main().catch((error) => {
  console.error(error);
  window.__mapReady = false;
});
</script>
</body>
</html>
"#;

fn parse_shape(json: &str) -> Result<ParsedShape> {
    let value: Value = serde_json::from_str(json).context("GeoJSON invalide")?;
    let kind = value
        .get("type")
        .and_then(|value| value.as_str())
        .ok_or_else(|| anyhow!("type GeoJSON manquant"))?;
    let coordinates = value
        .get("coordinates")
        .ok_or_else(|| anyhow!("coordonnées GeoJSON manquantes"))?;

    let mut shape = ParsedShape::default();
    match kind {
        "LineString" => {
            if let Some(line) = parse_line(coordinates) {
                shape.lines.push(line);
            }
        }
        "MultiLineString" => {
            if let Some(lines) = coordinates.as_array() {
                shape
                    .lines
                    .extend(lines.iter().filter_map(parse_line));
            }
        }
        "Polygon" => {
            if let Some(rings) = parse_polygon(coordinates) {
                shape.polygons.push(rings);
            }
        }
        "MultiPolygon" => {
            if let Some(polygons) = coordinates.as_array() {
                shape
                    .polygons
                    .extend(polygons.iter().filter_map(parse_polygon));
            }
        }
        "Point" => {
            if let Some(point) = parse_coord(coordinates) {
                shape.lines.push(vec![point]);
            }
        }
        _ => {}
    }
    Ok(shape)
}

fn parse_polygon(value: &Value) -> Option<Vec<Vec<[f64; 2]>>> {
    value
        .as_array()?
        .iter()
        .map(parse_line)
        .collect::<Option<Vec<_>>>()
}

fn parse_line(value: &Value) -> Option<Vec<[f64; 2]>> {
    let points: Vec<[f64; 2]> = value
        .as_array()?
        .iter()
        .filter_map(parse_coord)
        .collect();
    (points.len() >= 2).then_some(points)
}

fn parse_coord(value: &Value) -> Option<[f64; 2]> {
    let values = value.as_array()?;
    Some([values.first()?.as_f64()?, values.get(1)?.as_f64()?])
}

fn view_bounds(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> (f64, f64, f64, f64) {
    let report_width = (max_x - min_x).abs();
    let report_height = (max_y - min_y).abs();
    let center_x = (min_x + max_x) / 2.0;
    let center_y = (min_y + max_y) / 2.0;

    // On vise 300 m de large. Si la contribution est plus large, ou plus
    // haute que ne le permet le format de l'image, on élargit la vue pour
    // qu'elle reste entièrement visible.
    let aspect = WIDTH as f64 / HEIGHT as f64;
    let width = VIEW_WIDTH_METERS
        .max(report_width + 20.0)
        .max(report_height * aspect + 20.0);
    let height = width / aspect;

    (
        center_x - width / 2.0,
        center_y - height / 2.0,
        center_x + width / 2.0,
        center_y + height / 2.0,
    )
}

fn render_png(
    report: &ParsedShape,
    edges: &[(String, bool)],
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
) -> Result<Vec<u8>> {
    let mut image = RgbaImage::from_pixel(WIDTH, HEIGHT, BACKGROUND);
    let projection = Projection::new(min_x, min_y, max_x, max_y);
    let mut edge_shapes: Vec<(ParsedShape, bool)> = edges
        .iter()
        .filter_map(|(json, bicycle_route)| {
            parse_shape(json)
                .ok()
                .map(|shape| (shape, *bicycle_route))
        })
        .collect();
    edge_shapes.sort_by_key(|(_, bicycle_route)| *bicycle_route);

    for (shape, bicycle_route) in &edge_shapes {
        let color = if *bicycle_route { BIKE_ROUTE } else { ROAD };
        let width = if *bicycle_route { 3.0 } else { 1.6 };
        draw_shape_lines(&mut image, shape, &projection, color, width);
    }

    if report.polygons.is_empty() {
        draw_shape_lines(&mut image, report, &projection, REPORT_LINE, 5.0);
    } else {
        for polygon in &report.polygons {
            fill_polygon(&mut image, polygon, &projection, REPORT_FILL);
        }
        draw_shape_lines(&mut image, report, &projection, REPORT_LINE, 5.0);
    }

    if let Some((x, y)) = report.center() {
        let (px, py) = projection.point([x, y]);
        draw_disc(&mut image, px, py, 13.0, MARKER);
        draw_disc(&mut image, px, py, 5.0, MARKER_INNER);
    }

    draw_logo(&mut image);

    let mut output = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut output, ImageFormat::Png)
        .context("encodage PNG de la carte")?;
    Ok(output.into_inner())
}

struct Projection {
    min_x: f64,
    max_y: f64,
    scale: f64,
    offset_x: f64,
    offset_y: f64,
}

impl Projection {
    fn new(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Self {
        let width = (max_x - min_x).abs();
        let height = (max_y - min_y).abs();
        let width = if width < f64::EPSILON { 1.0 } else { width };
        let height = if height < f64::EPSILON { 1.0 } else { height };
        let scale_x = (WIDTH as f64 - 2.0 * PADDING) / width;
        let scale_y = (HEIGHT as f64 - 2.0 * PADDING) / height;
        let scale = scale_x.min(scale_y);
        let drawn_width = width * scale;
        let drawn_height = height * scale;
        Self {
            min_x,
            max_y,
            scale,
            offset_x: (WIDTH as f64 - drawn_width) / 2.0,
            offset_y: (HEIGHT as f64 - drawn_height) / 2.0,
        }
    }

    fn point(&self, point: [f64; 2]) -> (f64, f64) {
        (
            self.offset_x + (point[0] - self.min_x) * self.scale,
            self.offset_y + (self.max_y - point[1]) * self.scale,
        )
    }
}

fn draw_shape_lines(
    image: &mut RgbaImage,
    shape: &ParsedShape,
    projection: &Projection,
    color: Rgba<u8>,
    width: f64,
) {
    for line in &shape.lines {
        draw_polyline(image, line, projection, color, width);
    }
    for polygon in &shape.polygons {
        for ring in polygon {
            draw_polyline(image, ring, projection, color, width);
        }
    }
}

fn draw_polyline(
    image: &mut RgbaImage,
    points: &[[f64; 2]],
    projection: &Projection,
    color: Rgba<u8>,
    width: f64,
) {
    for pair in points.windows(2) {
        let (x0, y0) = projection.point(pair[0]);
        let (x1, y1) = projection.point(pair[1]);
        draw_segment(image, x0, y0, x1, y1, color, width);
    }
    if points.len() == 1 {
        let (x, y) = projection.point(points[0]);
        draw_disc(image, x, y, width.max(1.0) / 2.0, color);
    }
}

fn draw_segment(
    image: &mut RgbaImage,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    color: Rgba<u8>,
    width: f64,
) {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let steps = dx.abs().max(dy.abs()).ceil() as usize;
    if steps == 0 {
        draw_disc(image, x0, y0, width.max(1.0) / 2.0, color);
        return;
    }
    for index in 0..=steps {
        let t = index as f64 / steps as f64;
        draw_disc(
            image,
            x0 + dx * t,
            y0 + dy * t,
            width.max(1.0) / 2.0,
            color,
        );
    }
}

fn draw_disc(image: &mut RgbaImage, cx: f64, cy: f64, radius: f64, color: Rgba<u8>) {
    let min_x = (cx - radius).floor() as i64;
    let max_x = (cx + radius).ceil() as i64;
    let min_y = (cy - radius).floor() as i64;
    let max_y = (cy + radius).ceil() as i64;
    let radius_squared = radius * radius;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = x as f64 + 0.5 - cx;
            let dy = y as f64 + 0.5 - cy;
            if dx * dx + dy * dy <= radius_squared {
                blend_pixel(image, x, y, color);
            }
        }
    }
}

fn fill_polygon(
    image: &mut RgbaImage,
    rings: &[Vec<[f64; 2]>],
    projection: &Projection,
    color: Rgba<u8>,
) {
    let Some(min_y) = rings
        .iter()
        .flatten()
        .map(|point| projection.point(*point).1)
        .reduce(f64::min)
    else {
        return;
    };
    let Some(max_y) = rings
        .iter()
        .flatten()
        .map(|point| projection.point(*point).1)
        .reduce(f64::max)
    else {
        return;
    };

    let start_y = min_y.floor().max(0.0) as i64;
    let end_y = max_y.ceil().min(HEIGHT as f64) as i64;
    for y in start_y..=end_y {
        let scan_y = y as f64 + 0.5;
        let mut intersections = Vec::new();
        for ring in rings {
            for pair in ring.windows(2) {
                let (x0, y0) = projection.point(pair[0]);
                let (x1, y1) = projection.point(pair[1]);
                if (y0 <= scan_y && y1 > scan_y) || (y1 <= scan_y && y0 > scan_y) {
                    let ratio = (scan_y - y0) / (y1 - y0);
                    intersections.push(x0 + ratio * (x1 - x0));
                }
            }
        }
        intersections.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        for pair in intersections.chunks(2) {
            if pair.len() < 2 {
                break;
            }
            let start_x = pair[0].floor().max(0.0) as i64;
            let end_x = pair[1].ceil().min(WIDTH as f64) as i64;
            for x in start_x..=end_x {
                blend_pixel(image, x, y, color);
            }
        }
    }
}

fn blend_pixel(image: &mut RgbaImage, x: i64, y: i64, color: Rgba<u8>) {
    if x < 0 || y < 0 || x >= image.width() as i64 || y >= image.height() as i64 {
        return;
    }
    let pixel = image.get_pixel_mut(x as u32, y as u32);
    let alpha = color[3] as f64 / 255.0;
    for channel in 0..3 {
        pixel[channel] = (color[channel] as f64 * alpha
            + pixel[channel] as f64 * (1.0 - alpha))
            .round() as u8;
    }
    pixel[3] = 255;
}

fn draw_logo(image: &mut RgbaImage) {
    let Ok(logo) = image::open("pub/logo-big.png") else {
        return;
    };
    let logo = logo.resize(72, 72, FilterType::Lanczos3).to_rgba8();
    let x = 20i64;
    let y = 20i64;
    for offset_y in -6..78 {
        for offset_x in -6..78 {
            blend_pixel(image, x + offset_x, y + offset_y, Rgba([255, 255, 255, 225]));
        }
    }
    for (sx, sy, pixel) in logo.enumerate_pixels() {
        let [red, green, blue, alpha] = pixel.0;
        if alpha > 0 {
            blend_pixel(
                image,
                x + sx as i64,
                y + sy as i64,
                Rgba([red, green, blue, alpha]),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_vue_fait_400_metres_de_large() {
        let (min_x, min_y, max_x, max_y) = view_bounds(0.0, 0.0, 20.0, 20.0);
        assert!((max_x - min_x - 400.0).abs() < 1e-9);
        assert!(
            (max_y - min_y - 400.0 * HEIGHT as f64 / WIDTH as f64).abs() < 1e-9
        );

        // Une contribution plus large que 400 m élargit la vue.
        let (min_x, _, max_x, _) = view_bounds(0.0, 0.0, 500.0, 20.0);
        assert!((max_x - min_x - 520.0).abs() < 1e-9);
    }

    #[test]
    fn genere_une_image_png() {
        let report = parse_shape(
            r#"{"type":"Polygon","coordinates":[[[-73.60,45.50],[-73.59,45.50],[-73.59,45.51],[-73.60,45.51],[-73.60,45.50]]]}"#,
        )
        .unwrap();
        let edges = vec![(
            r#"{"type":"LineString","coordinates":[[-73.61,45.49],[-73.58,45.52]]}"#.to_string(),
            true,
        )];
        let png = render_png(&report, &edges, -73.62, 45.48, -73.57, 45.53).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let image = image::load_from_memory(&png).unwrap();
        assert_eq!((image.width(), image.height()), (WIDTH, HEIGHT));
    }
}
