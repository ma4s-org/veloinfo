import * as maplibregl from '/node_modules/maplibre-gl/dist/maplibre-gl.mjs';

const html = String.raw;

const REFRESH_MS = 15000;
const RECENT_SECONDS = 24 * 60 * 60;
const LIST_LIMIT = 500;
const ROUTE_PREVIEW_CACHE_LIMIT = 100;

const numberFormat = new Intl.NumberFormat('fr-CA');
const dateTimeFormat = new Intl.DateTimeFormat('fr-CA', {
  dateStyle: 'short',
  timeStyle: 'medium',
});
const timeFormat = new Intl.DateTimeFormat('fr-CA', {
  hour: '2-digit',
  minute: '2-digit',
  second: '2-digit',
});

function escapeHtml(value) {
  return String(value ?? '')
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;');
}

function relativeTime(iso) {
  if (!iso) return 'inconnue';
  const seconds = Math.max(0, (Date.now() - new Date(iso).getTime()) / 1000);
  if (seconds < 10) return "à l'instant";
  if (seconds < 60) return `il y a ${Math.round(seconds)} s`;
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `il y a ${minutes} min`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `il y a ${hours} h`;
  return `il y a ${Math.round(hours / 24)} j`;
}

function formatDuration(seconds) {
  const total = Math.max(0, Math.round(seconds || 0));
  if (total < 60) return `${total} s`;
  const minutes = Math.floor(total / 60);
  const remainingSeconds = total % 60;
  if (minutes < 60) {
    return remainingSeconds ? `${minutes} min ${remainingSeconds} s` : `${minutes} min`;
  }
  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;
  return remainingMinutes ? `${hours} h ${remainingMinutes} min` : `${hours} h`;
}

function describeClient(session) {
  const parts = [session.device, session.browser, session.os].filter(Boolean);
  return parts.length ? parts.join(' · ') : 'Appareil inconnu';
}

function positionText(session) {
  if (session.lat == null || session.lng == null) return 'Position inconnue';
  const zoom = session.zoom == null ? '?' : Math.round(session.zoom);
  return `${session.lat.toFixed(3)}, ${session.lng.toFixed(3)} · zoom ${zoom}`;
}

function sessionPlace(session) {
  if (session.default_view) return 'Vue par défaut';
  return session.city || positionText(session);
}

function referrerText(session) {
  if (!session.referrer) return null;
  try {
    return new URL(session.referrer).hostname;
  } catch {
    return session.referrer;
  }
}

const dayLabelFormat = new Intl.DateTimeFormat('fr-CA', {
  day: 'numeric',
  month: 'short',
});
const monthLabelFormat = new Intl.DateTimeFormat('fr-CA', {
  month: 'short',
  year: '2-digit',
});
const longDateFormat = new Intl.DateTimeFormat('fr-CA', {
  day: 'numeric',
  month: 'long',
  year: 'numeric',
});
const longMonthFormat = new Intl.DateTimeFormat('fr-CA', {
  month: 'long',
  year: 'numeric',
});
const longWeekdayFormat = new Intl.DateTimeFormat('fr-CA', {
  weekday: 'long',
  day: 'numeric',
  month: 'long',
  year: 'numeric',
});

function parseBucketDate(bucket) {
  // Midi local pour éviter tout décalage de fuseau sur une date seule.
  return new Date(`${bucket}T12:00:00`);
}

function formatBucketLabel(bucket, period) {
  const date = parseBucketDate(bucket);
  return period === 'month'
    ? monthLabelFormat.format(date)
    : dayLabelFormat.format(date);
}

function formatBucketFull(bucket, period) {
  const date = parseBucketDate(bucket);
  if (period === 'month') return longMonthFormat.format(date);
  if (period === 'week') return `Semaine du ${longDateFormat.format(date)}`;
  return longWeekdayFormat.format(date);
}

function niceMax(value) {
  if (value <= 4) return 4;
  return Math.ceil(value / 4) * 4;
}

function safeImageUrl(value) {
  if (!value) return null;
  const path = String(value);
  if (path.startsWith('/images/')
      || path.startsWith('https://')
      || path.startsWith('http://')) {
    return path;
  }
  return `/images/${path}`;
}

function routeStatusLabel(status) {
  return {
    requested: 'Demandé',
    following: 'Suivi',
    completed: 'Terminé',
    abandoned: 'Abandonné',
  }[status] || status;
}

function routeTypeLabel(type) {
  if (type === 'safe') return 'Sécurisé';
  if (type === 'fast') return 'Rapide';
  return '—';
}

function formatKm(value) {
  return Number.isFinite(value)
    ? `${numberFormat.format(Math.round(value * 10) / 10)} km`
    : '—';
}

function scoreLabel(score) {
  return Number.isFinite(score) && score >= 0 && score <= 1
    ? `${Math.round(score * 100)} %`
    : '—';
}

function sourceLabel(session) {
  if (session.source !== 'matomo') return null;
  if (session.default_view) return 'Import Matomo — centre de carte par défaut';
  return session.zoom != null
    ? 'Import Matomo — centre de carte récupéré de l’URL'
    : 'Import Matomo — position approximative par IP, zoom inconnu';
}

function buildPopupHtml(session) {
  return html`
    <div class="usage-popup">
      <div class="usage-popup-head">
        <strong>Session ${escapeHtml(session.id.slice(0, 8))}</strong>
        <span class="usage-badge ${session.live ? 'live' : ''}">
          ${session.live ? 'En direct' : 'Terminée'}
        </span>
      </div>
      <div>${escapeHtml(describeClient(session))}</div>
      <div>${escapeHtml(formatDuration(session.duration_seconds))} · ${numberFormat.format(session.page_views)} page(s)</div>
      <div>${escapeHtml(sessionPlace(session))}</div>
      ${sourceLabel(session)
        ? `<div class="usage-muted">${escapeHtml(sourceLabel(session))}</div>`
        : ''}
      <div class="usage-muted">${escapeHtml(relativeTime(session.last_seen))}</div>
    </div>
  `;
}

class ViUsage extends HTMLElement {
  constructor() {
    super();
    this.filter = 'live';
    this.sessions = [];
    this.stats = null;
    this.selected = null;
    this.popup = null;
    this.ignorePopupClose = false;
    this.refreshTimer = null;
    this.hasFitBounds = false;
    this.requestId = 0;
    this.lastFetchAt = 0;
    this.trafficPeriod = 'day';
    this.traffic = null;
    this.trafficRequestId = 0;
    this.lastTrafficAt = 0;
    this.routeWindow = '24h';
    this.routes = null;
    this.routesRequestId = 0;
    this.lastRoutesAt = 0;
    this.contributionWindow = 'all';
    this.contributions = null;
    this.contributionsRequestId = 0;
    this.lastContributionsAt = 0;
    this.onResize = null;
    this.routePreview = null;
    this.routePreviewMap = null;
    this.routePreviewReady = false;
    this.routePreviewCoords = null;
    this.routePreviewFound = false;
    this.routePreviewRouteId = null;
    this.routePreviewTimer = null;
    this.routePreviewHideTimer = null;
    this.routePreviewToken = 0;
    this.routePreviewAbort = null;
    this.routePreviewCache = new Map();
  }

  connectedCallback() {
    const prefersDark = window.matchMedia
      && window.matchMedia('(prefers-color-scheme: dark)').matches;
    document.documentElement.setAttribute('data-theme', prefersDark ? 'dark' : 'light');

    const params = new URLSearchParams(window.location.search);
    const requested = params.get('window');
    if (['live', '24h', '7d', 'all'].includes(requested)) {
      this.filter = requested;
    }
    const requestedPeriod = params.get('period');
    if (['day', 'week', 'month'].includes(requestedPeriod)) {
      this.trafficPeriod = requestedPeriod;
    }

    this.render();
    this.initMap();
    this.refresh();
    this.refreshTraffic();
    this.refreshRoutes();
    this.refreshContributions();

    this.onResize = () => {
      if (this.traffic) this.renderTraffic();
      this.hideRoutePreview();
    };
    window.addEventListener('resize', this.onResize);

    this.refreshTimer = window.setInterval(() => {
      if (document.hidden) return;
      // Les périodes historiques changent lentement : on évite de renvoyer
      // 15 000 sessions toutes les 15 secondes.
      const interval = this.filter === 'live'
        ? REFRESH_MS
        : (this.filter === 'all' ? 300000 : 60000);
      if (Date.now() - this.lastFetchAt >= interval) this.refresh();
    }, REFRESH_MS);

    document.addEventListener('visibilitychange', () => {
      if (!document.hidden) this.refresh();
    });
  }

  disconnectedCallback() {
    window.clearInterval(this.refreshTimer);
    if (this.onResize) window.removeEventListener('resize', this.onResize);
    if (this.onPreviewScroll) window.removeEventListener('scroll', this.onPreviewScroll, true);
    this.cancelRoutePreview();
    window.clearTimeout(this.routePreviewHideTimer);
    if (this.routePreviewAbort) this.routePreviewAbort.abort();
    if (this.routePreviewMap) {
      this.routePreviewMap.remove();
      this.routePreviewMap = null;
    }
  }

  render() {
    this.innerHTML = html`
      <style>
        vi-usage {
          display: block;
          min-height: 100vh;
          padding-bottom: 16px;
          color: var(--vi-text, rgb(24 24 27));
          background: var(--vi-bg, rgb(244 244 245));
          font-family: Roboto, system-ui, -apple-system, "Segoe UI", sans-serif;
        }

        vi-usage .usage-topbar {
          display: flex;
          align-items: flex-start;
          justify-content: space-between;
          gap: 16px;
          padding: 20px 24px 12px;
        }

        vi-usage h1 {
          font-size: clamp(1.35rem, 2.5vw, 1.9rem);
          font-weight: 700;
          letter-spacing: -0.02em;
        }

        vi-usage .usage-subtitle {
          margin-top: 4px;
          color: var(--vi-text-muted, rgb(82 82 91));
          font-size: 0.9rem;
          max-width: 62ch;
        }

        vi-usage .usage-topbar-actions {
          display: flex;
          align-items: center;
          gap: 16px;
          flex-shrink: 0;
          padding-top: 4px;
        }

        vi-usage .usage-refresh {
          display: inline-flex;
          align-items: center;
          gap: 7px;
          color: var(--vi-text-muted, rgb(82 82 91));
          font-size: 0.82rem;
          white-space: nowrap;
        }

        vi-usage .usage-pulse {
          width: 8px;
          height: 8px;
          border-radius: 50%;
          background: rgb(22 163 74);
          box-shadow: 0 0 0 0 rgb(22 163 74 / 0.6);
          animation: usage-pulse 2s infinite;
        }

        vi-usage .usage-refresh.error .usage-pulse {
          background: rgb(220 38 38);
          animation: none;
        }

        @keyframes usage-pulse {
          0% { box-shadow: 0 0 0 0 rgb(22 163 74 / 0.55); }
          70% { box-shadow: 0 0 0 8px rgb(22 163 74 / 0); }
          100% { box-shadow: 0 0 0 0 rgb(22 163 74 / 0); }
        }

        vi-usage .usage-back {
          color: var(--vi-text, rgb(24 24 27));
          text-decoration: none;
          font-size: 0.9rem;
          font-weight: 500;
          padding: 8px 12px;
          border: 1px solid var(--vi-border, rgb(212 212 216));
          border-radius: 999px;
          background: var(--vi-surface, white);
          transition: background 0.15s ease;
        }

        vi-usage .usage-back:hover {
          background: var(--vi-surface-2, rgb(244 244 245));
        }

        vi-usage .usage-stats {
          display: grid;
          grid-template-columns: repeat(auto-fit, minmax(145px, 1fr));
          gap: 12px;
          padding: 4px 24px 16px;
        }

        vi-usage .usage-stat {
          position: relative;
          display: flex;
          flex-direction: column;
          gap: 2px;
          padding: 14px 16px;
          border: 1px solid var(--vi-border, rgb(228 228 231));
          border-radius: 14px;
          background: var(--vi-surface, white);
          overflow: hidden;
        }

        vi-usage .usage-stat::after {
          content: "";
          position: absolute;
          inset: auto 0 0 0;
          height: 3px;
          background: var(--vi-surface-2, rgb(228 228 231));
        }

        vi-usage .usage-stat.live::after { background: rgb(22 163 74); }
        vi-usage .usage-stat.pages::after { background: rgb(37 99 235); }
        vi-usage .usage-stat.actions::after { background: rgb(245 158 11); }

        vi-usage .usage-stat-value {
          font-size: 1.65rem;
          font-weight: 700;
          line-height: 1.1;
          font-variant-numeric: tabular-nums;
        }

        vi-usage .usage-stat-label {
          font-size: 0.85rem;
          font-weight: 500;
        }

        vi-usage .usage-stat-hint {
          color: var(--vi-text-muted, rgb(113 113 122));
          font-size: 0.72rem;
        }

        vi-usage {
          --usage-bar: rgb(37 99 235);
          --usage-bar-hover: rgb(29 78 216);
        }

        :root[data-theme="dark"] vi-usage {
          --usage-bar: rgb(96 165 250);
          --usage-bar-hover: rgb(147 197 253);
        }

        vi-usage .usage-traffic-card {
          margin: 0 24px 16px;
          padding: 14px 16px 10px;
          border: 1px solid var(--vi-border, rgb(228 228 231));
          border-radius: 16px;
          background: var(--vi-surface, white);
        }

        vi-usage .usage-traffic-head {
          display: flex;
          align-items: center;
          justify-content: space-between;
          gap: 12px;
          flex-wrap: wrap;
          margin-bottom: 10px;
        }

        vi-usage .usage-traffic-title {
          display: flex;
          align-items: baseline;
          gap: 10px;
          flex-wrap: wrap;
          min-width: 0;
        }

        vi-usage .usage-traffic-title h2 {
          font-size: 1rem;
          font-weight: 600;
        }

        vi-usage .usage-traffic-total {
          color: var(--vi-text-muted, rgb(82 82 91));
          font-size: 0.78rem;
        }

        vi-usage .usage-chart {
          position: relative;
          width: 100%;
          min-height: 200px;
        }

        vi-usage .usage-chart svg {
          display: block;
          width: 100%;
          height: auto;
          overflow: visible;
        }

        vi-usage .usage-chart-bar {
          fill: var(--usage-bar, rgb(37 99 235));
        }

        vi-usage .usage-chart-bar:hover {
          fill: var(--usage-bar-hover, rgb(29 78 216));
        }

        vi-usage .usage-chart-axis {
          fill: var(--vi-text-muted, rgb(113 113 122));
          font-size: 11px;
          font-family: inherit;
        }

        vi-usage .usage-chart-grid {
          stroke: var(--vi-border, rgb(228 228 231));
          stroke-width: 1;
        }

        vi-usage .usage-chart-empty {
          position: absolute;
          inset: 0;
          display: flex;
          align-items: center;
          justify-content: center;
          color: var(--vi-text-muted, rgb(82 82 91));
          font-size: 0.85rem;
          pointer-events: none;
        }

        vi-usage .usage-insights {
          display: grid;
          grid-template-columns: repeat(2, minmax(0, 1fr));
          gap: 16px;
          padding: 0 24px 16px;
        }

        vi-usage .usage-insight-card {
          min-width: 0;
          padding: 14px 16px;
          border: 1px solid var(--vi-border, rgb(228 228 231));
          border-radius: 16px;
          background: var(--vi-surface, white);
        }

        vi-usage .usage-insight-head {
          display: flex;
          align-items: center;
          justify-content: space-between;
          gap: 12px;
          flex-wrap: wrap;
          margin-bottom: 10px;
        }

        vi-usage .usage-mini-stats {
          display: flex;
          flex-wrap: wrap;
          gap: 8px;
          margin-bottom: 10px;
        }

        vi-usage .usage-mini-stat {
          display: inline-flex;
          align-items: baseline;
          gap: 5px;
          padding: 5px 9px;
          border-radius: 999px;
          background: var(--vi-surface-2, rgb(244 244 245));
          color: var(--vi-text-muted, rgb(82 82 91));
          font-size: 0.74rem;
        }

        vi-usage .usage-mini-stat strong {
          color: var(--vi-text, rgb(24 24 27));
          font-size: 0.85rem;
        }

        vi-usage .usage-table-wrap {
          overflow: auto;
          max-height: 360px;
        }

        vi-usage .usage-table {
          width: 100%;
          border-collapse: collapse;
          font-size: 0.75rem;
        }

        vi-usage .usage-table th {
          position: sticky;
          top: 0;
          z-index: 1;
          padding: 6px 8px;
          border-bottom: 1px solid var(--vi-border, rgb(228 228 231));
          background: var(--vi-surface, white);
          color: var(--vi-text-muted, rgb(82 82 91));
          font-weight: 500;
          text-align: left;
          white-space: nowrap;
        }

        vi-usage .usage-table td {
          padding: 7px 8px;
          border-bottom: 1px solid var(--vi-border, rgb(228 228 231));
          vertical-align: top;
        }

        vi-usage .usage-table tr:last-child td {
          border-bottom: 0;
        }

        vi-usage .usage-route-row:hover,
        vi-usage .usage-route-row:focus-visible {
          background: var(--vi-surface-2, rgb(244 244 245));
        }

        vi-usage .usage-route-row:focus-visible {
          outline: 2px solid var(--usage-bar, rgb(37 99 235));
          outline-offset: -2px;
        }

        vi-usage .usage-route-trajet {
          font-weight: 500;
          white-space: nowrap;
        }

        vi-usage .usage-route-preview {
          position: fixed;
          z-index: 70;
          width: min(340px, calc(100vw - 16px));
          padding: 10px;
          border: 1px solid var(--vi-border, rgb(228 228 231));
          border-radius: 12px;
          background: var(--vi-surface, white);
          color: var(--vi-text, rgb(24 24 27));
          box-shadow: 0 14px 40px rgb(0 0 0 / 0.22);
          pointer-events: none;
        }

        vi-usage .usage-route-preview[hidden] {
          display: none;
        }

        vi-usage .usage-route-preview-head {
          display: flex;
          align-items: baseline;
          justify-content: space-between;
          gap: 8px;
          margin-bottom: 7px;
          font-size: 0.78rem;
        }

        vi-usage .usage-route-preview-head strong {
          font-weight: 600;
        }

        vi-usage .usage-route-preview-meta {
          color: var(--vi-text-muted, rgb(113 113 122));
          font-size: 0.72rem;
          white-space: nowrap;
        }

        vi-usage #usage-route-preview-map {
          height: 170px;
          border-radius: 8px;
          overflow: hidden;
          background: var(--vi-surface-2, rgb(244 244 245));
        }

        vi-usage .usage-route-preview-note {
          margin-top: 6px;
          min-height: 1em;
          font-size: 0.72rem;
        }

        @media (hover: none) {
          vi-usage .usage-route-preview {
            display: none;
          }
        }

        vi-usage .usage-progress {
          display: flex;
          align-items: center;
          gap: 6px;
          min-width: 80px;
        }

        vi-usage .usage-progress-track {
          width: 48px;
          height: 6px;
          border-radius: 999px;
          background: var(--vi-surface-2, rgb(228 228 231));
          overflow: hidden;
        }

        vi-usage .usage-progress-fill {
          display: block;
          height: 100%;
          background: var(--usage-bar, rgb(37 99 235));
        }

        vi-usage .usage-badge.completed {
          border-color: rgb(22 163 74 / 0.35);
          background: rgb(22 163 74 / 0.1);
          color: rgb(21 128 61);
        }

        vi-usage .usage-badge.abandoned {
          border-color: rgb(220 38 38 / 0.35);
          background: rgb(220 38 38 / 0.1);
          color: rgb(185 28 28);
        }

        vi-usage .usage-badge.following {
          border-color: rgb(37 99 235 / 0.35);
          background: rgb(37 99 235 / 0.1);
          color: rgb(29 78 216);
        }

        vi-usage .usage-contributors {
          display: flex;
          flex-wrap: wrap;
          gap: 6px;
          margin-bottom: 10px;
        }

        vi-usage .usage-contributor {
          padding: 4px 8px;
          border-radius: 999px;
          background: var(--vi-surface-2, rgb(244 244 245));
          color: var(--vi-text-muted, rgb(82 82 91));
          font-size: 0.72rem;
        }

        vi-usage .usage-contributor strong {
          color: var(--vi-text, rgb(24 24 27));
        }

        vi-usage .usage-contrib-list {
          display: flex;
          flex-direction: column;
          gap: 2px;
          max-height: 360px;
          overflow: auto;
        }

        vi-usage .usage-contrib-item {
          display: flex;
          gap: 10px;
          padding: 8px;
          border-radius: 10px;
        }

        vi-usage .usage-contrib-item:hover {
          background: var(--vi-surface-2, rgb(244 244 245));
        }

        vi-usage .usage-contrib-thumb {
          flex: 0 0 auto;
          width: 42px;
          height: 42px;
          border-radius: 8px;
          object-fit: cover;
        }

        vi-usage .usage-contrib-body {
          min-width: 0;
          flex: 1;
        }

        vi-usage .usage-contrib-head {
          display: flex;
          align-items: baseline;
          gap: 6px;
          flex-wrap: wrap;
          font-size: 0.78rem;
        }

        vi-usage .usage-contrib-head .usage-muted {
          font-size: 0.72rem;
        }

        vi-usage .usage-contrib-text {
          margin-top: 2px;
          font-size: 0.78rem;
          line-height: 1.4;
          overflow-wrap: anywhere;
          white-space: pre-wrap;
        }

        vi-usage .usage-contrib-link {
          display: inline-block;
          margin-top: 3px;
          color: var(--vi-text-muted, rgb(113 113 122));
          font-size: 0.72rem;
        }

        vi-usage .usage-contrib-link:hover {
          color: var(--vi-text, rgb(24 24 27));
        }

        vi-usage .usage-layout {
          display: grid;
          grid-template-columns: minmax(0, 1fr) 390px;
          gap: 16px;
          padding: 0 24px;
        }

        vi-usage .usage-map-card,
        vi-usage .usage-sessions-card {
          border: 1px solid var(--vi-border, rgb(228 228 231));
          border-radius: 16px;
          background: var(--vi-surface, white);
          overflow: hidden;
          min-width: 0;
        }

        vi-usage .usage-map-card {
          display: flex;
          flex-direction: column;
        }

        vi-usage .usage-map-toolbar {
          display: flex;
          align-items: center;
          justify-content: space-between;
          gap: 12px;
          padding: 10px 12px;
          border-bottom: 1px solid var(--vi-border, rgb(228 228 231));
        }

        vi-usage .usage-segmented {
          display: inline-flex;
          padding: 3px;
          gap: 2px;
          border-radius: 999px;
          background: var(--vi-surface-2, rgb(244 244 245));
        }

        vi-usage .usage-segmented button {
          border: 0;
          border-radius: 999px;
          padding: 6px 12px;
          background: transparent;
          color: var(--vi-text-muted, rgb(82 82 91));
          font: inherit;
          font-size: 0.8rem;
          font-weight: 500;
          cursor: pointer;
          transition: background 0.15s ease, color 0.15s ease;
        }

        vi-usage .usage-segmented button:hover {
          color: var(--vi-text, rgb(24 24 27));
        }

        vi-usage .usage-segmented button.active {
          background: var(--vi-surface, white);
          color: var(--vi-text, rgb(24 24 27));
          box-shadow: 0 1px 3px rgb(0 0 0 / 0.12);
        }

        vi-usage .usage-map-note {
          color: var(--vi-text-muted, rgb(82 82 91));
          font-size: 0.78rem;
          white-space: nowrap;
        }

        vi-usage .usage-map-wrap {
          position: relative;
          flex: 1;
        }

        vi-usage #usage-map {
          height: clamp(420px, calc(100vh - 330px), 880px);
          min-height: 420px;
        }

        vi-usage .usage-empty {
          position: absolute;
          inset: 0;
          display: flex;
          align-items: center;
          justify-content: center;
          pointer-events: none;
          padding: 24px;
          text-align: center;
          color: var(--vi-text-muted, rgb(82 82 91));
          font-size: 0.92rem;
          background: color-mix(in srgb, var(--vi-surface, white) 55%, transparent);
        }

        vi-usage .usage-empty[hidden] {
          display: none;
        }

        vi-usage .usage-legend {
          position: absolute;
          left: 10px;
          bottom: 10px;
          display: flex;
          flex-direction: column;
          gap: 4px;
          padding: 7px 9px;
          border: 1px solid var(--vi-border, rgb(228 228 231));
          border-radius: 8px;
          background: color-mix(in srgb, var(--vi-surface, white) 92%, transparent);
          color: var(--vi-text-muted, rgb(82 82 91));
          font-size: 0.72rem;
          pointer-events: none;
        }

        vi-usage .usage-legend span {
          display: inline-flex;
          align-items: center;
          gap: 6px;
        }

        vi-usage .usage-legend i {
          display: inline-block;
          width: 9px;
          height: 9px;
          border-radius: 50%;
        }

        vi-usage .usage-sessions-card {
          display: flex;
          flex-direction: column;
          max-height: calc(100vh - 250px);
          min-height: 420px;
        }

        vi-usage .usage-sessions-head {
          display: flex;
          align-items: baseline;
          justify-content: space-between;
          gap: 8px;
          padding: 14px 16px 10px;
          border-bottom: 1px solid var(--vi-border, rgb(228 228 231));
        }

        vi-usage .usage-sessions-head h2 {
          font-size: 1rem;
          font-weight: 600;
        }

        vi-usage .usage-sessions-count {
          color: var(--vi-text-muted, rgb(82 82 91));
          font-size: 0.78rem;
        }

        vi-usage .usage-sessions-list {
          display: flex;
          flex-direction: column;
          gap: 2px;
          padding: 6px;
          overflow-y: auto;
          flex: 1;
        }

        vi-usage .usage-empty-list {
          padding: 28px 16px;
          text-align: center;
          color: var(--vi-text-muted, rgb(82 82 91));
          font-size: 0.88rem;
        }

        vi-usage .usage-session {
          display: flex;
          align-items: flex-start;
          gap: 10px;
          width: 100%;
          padding: 9px 10px;
          border: 0;
          border-radius: 10px;
          background: transparent;
          color: inherit;
          font: inherit;
          text-align: left;
          cursor: pointer;
        }

        vi-usage .usage-session:hover {
          background: var(--vi-surface-2, rgb(244 244 245));
        }

        vi-usage .usage-session.selected {
          background: color-mix(in srgb, rgb(37 99 235) 10%, var(--vi-surface, white));
          outline: 1px solid rgb(37 99 235 / 0.35);
        }

        vi-usage .usage-session-dot {
          flex: 0 0 auto;
          width: 9px;
          height: 9px;
          margin-top: 5px;
          border-radius: 50%;
          background: rgb(148 163 184);
        }

        vi-usage .usage-session.live .usage-session-dot {
          background: rgb(22 163 74);
        }

        vi-usage .usage-session.recent .usage-session-dot {
          background: rgb(37 99 235);
        }

        vi-usage .usage-session-body {
          display: flex;
          flex-direction: column;
          gap: 2px;
          min-width: 0;
          flex: 1;
        }

        vi-usage .usage-session-top {
          display: flex;
          align-items: baseline;
          justify-content: space-between;
          gap: 8px;
        }

        vi-usage .usage-session-client {
          font-size: 0.82rem;
          font-weight: 600;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }

        vi-usage .usage-session-ago {
          flex: 0 0 auto;
          color: var(--vi-text-muted, rgb(113 113 122));
          font-size: 0.72rem;
        }

        vi-usage .usage-session-meta {
          color: var(--vi-text-muted, rgb(82 82 91));
          font-size: 0.74rem;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }

        vi-usage .usage-detail {
          margin: 6px;
          padding: 12px 14px;
          border: 1px solid var(--vi-border, rgb(228 228 231));
          border-radius: 12px;
          background: var(--vi-surface-2, rgb(250 250 250));
        }

        vi-usage .usage-detail[hidden] {
          display: none;
        }

        vi-usage .usage-detail-head {
          display: flex;
          align-items: center;
          justify-content: space-between;
          gap: 8px;
          margin-bottom: 8px;
        }

        vi-usage .usage-detail-head strong {
          font-size: 0.9rem;
        }

        vi-usage .usage-detail-close {
          border: 0;
          background: transparent;
          color: var(--vi-text-muted, rgb(82 82 91));
          font-size: 1rem;
          cursor: pointer;
          padding: 2px 6px;
          border-radius: 6px;
        }

        vi-usage .usage-detail-close:hover {
          background: var(--vi-surface, white);
        }

        vi-usage .usage-badge {
          display: inline-flex;
          align-items: center;
          padding: 2px 7px;
          border-radius: 999px;
          background: var(--vi-surface, white);
          border: 1px solid var(--vi-border, rgb(228 228 231));
          color: var(--vi-text-muted, rgb(82 82 91));
          font-size: 0.68rem;
          font-weight: 600;
          white-space: nowrap;
        }

        vi-usage .usage-badge.live {
          border-color: rgb(22 163 74 / 0.35);
          background: rgb(22 163 74 / 0.1);
          color: rgb(21 128 61);
        }

        vi-usage .usage-detail dl {
          display: grid;
          grid-template-columns: minmax(90px, auto) minmax(0, 1fr);
          gap: 4px 10px;
          font-size: 0.78rem;
        }

        vi-usage .usage-detail dt {
          color: var(--vi-text-muted, rgb(113 113 122));
        }

        vi-usage .usage-detail dd {
          margin: 0;
          overflow-wrap: anywhere;
        }

        vi-usage .usage-muted {
          color: var(--vi-text-muted, rgb(113 113 122));
        }

        vi-usage .maplibregl-popup-content {
          padding: 10px 12px;
          border-radius: 10px;
          font-family: inherit;
          font-size: 0.78rem;
          line-height: 1.45;
          background: var(--vi-surface, white);
          color: var(--vi-text, rgb(24 24 27));
          box-shadow: 0 3px 16px rgb(0 0 0 / 0.25);
        }

        vi-usage .maplibregl-popup-close-button {
          padding: 0 6px;
          color: var(--vi-text-muted, rgb(82 82 91));
          font-size: 18px;
          line-height: 1.2;
          border-radius: 6px;
        }

        vi-usage .maplibregl-popup-close-button:hover {
          background: var(--vi-surface-2, rgb(244 244 245));
          color: var(--vi-text, rgb(24 24 27));
        }

        vi-usage .maplibregl-popup-anchor-top .maplibregl-popup-tip {
          border-bottom-color: var(--vi-surface, white);
        }

        vi-usage .maplibregl-popup-anchor-bottom .maplibregl-popup-tip {
          border-top-color: var(--vi-surface, white);
        }

        vi-usage .maplibregl-popup-anchor-left .maplibregl-popup-tip {
          border-right-color: var(--vi-surface, white);
        }

        vi-usage .maplibregl-popup-anchor-right .maplibregl-popup-tip {
          border-left-color: var(--vi-surface, white);
        }

        vi-usage .usage-popup {
          display: flex;
          flex-direction: column;
          gap: 3px;
        }

        vi-usage .usage-popup-head {
          display: flex;
          align-items: center;
          justify-content: space-between;
          gap: 10px;
          margin-bottom: 3px;
        }

        @media (max-width: 1040px) {
          vi-usage .usage-layout,
          vi-usage .usage-insights {
            grid-template-columns: 1fr;
          }

          vi-usage .usage-sessions-card {
            max-height: none;
          }

          vi-usage .usage-sessions-list {
            max-height: 45vh;
          }

          vi-usage #usage-map {
            height: 55vh;
          }
        }

        @media (max-width: 640px) {
          vi-usage .usage-topbar {
            flex-direction: column;
            padding: 16px 16px 10px;
          }

          vi-usage .usage-topbar-actions {
            width: 100%;
            justify-content: space-between;
          }

          vi-usage .usage-stats,
          vi-usage .usage-layout {
            padding-left: 16px;
            padding-right: 16px;
          }

          vi-usage .usage-traffic-card {
            margin-left: 16px;
            margin-right: 16px;
          }

          vi-usage .usage-insights {
            padding-left: 16px;
            padding-right: 16px;
          }

          vi-usage .usage-stats {
            grid-template-columns: repeat(2, minmax(0, 1fr));
          }

          vi-usage .usage-map-toolbar {
            flex-wrap: wrap;
          }
        }
      </style>

      <header class="usage-topbar">
        <div>
          <h1>Usage de Vélo info</h1>
          <p class="usage-subtitle">
            Où les gens consultent la carte et combien de temps ils restent.
            Aucune adresse IP ni position GPS n'est enregistrée : seule la zone
            affichée sur la carte est conservée.
          </p>
        </div>
        <div class="usage-topbar-actions">
          <span class="usage-refresh" id="usage-refresh">
            <span class="usage-pulse" aria-hidden="true"></span>
            <span id="usage-refresh-text">Mise à jour…</span>
          </span>
          <a class="usage-back" href="/">← Retour à la carte</a>
        </div>
      </header>

      <section class="usage-stats" aria-label="Statistiques">
        <article class="usage-stat live">
          <span class="usage-stat-value" id="usage-stat-live">–</span>
          <span class="usage-stat-label">En direct</span>
          <span class="usage-stat-hint">5 dernières minutes</span>
        </article>
        <article class="usage-stat">
          <span class="usage-stat-value" id="usage-stat-24h">–</span>
          <span class="usage-stat-label">Sessions</span>
          <span class="usage-stat-hint">Dernières 24 heures</span>
        </article>
        <article class="usage-stat pages">
          <span class="usage-stat-value" id="usage-stat-pages">–</span>
          <span class="usage-stat-label">Pages vues</span>
          <span class="usage-stat-hint">Dernières 24 heures</span>
        </article>
        <article class="usage-stat actions">
          <span class="usage-stat-value" id="usage-stat-actions">–</span>
          <span class="usage-stat-label">Actions</span>
          <span class="usage-stat-hint">Dernières 24 heures</span>
        </article>
        <article class="usage-stat">
          <span class="usage-stat-value" id="usage-stat-duration">–</span>
          <span class="usage-stat-label">Durée moyenne</span>
          <span class="usage-stat-hint">Sessions des 24 h</span>
        </article>
        <article class="usage-stat">
          <span class="usage-stat-value" id="usage-stat-total">–</span>
          <span class="usage-stat-label">Sessions totales</span>
          <span class="usage-stat-hint">Depuis le début</span>
        </article>
      </section>

      <section class="usage-traffic-card" aria-label="Trafic">
        <div class="usage-traffic-head">
          <div class="usage-traffic-title">
            <h2>Trafic</h2>
            <span class="usage-traffic-total" id="usage-traffic-total"></span>
          </div>
          <div class="usage-segmented" id="usage-traffic-periods" role="group" aria-label="Granularité du trafic">
            <button type="button" data-period="day" class="active">Par jour</button>
            <button type="button" data-period="week">Par semaine</button>
            <button type="button" data-period="month">Par mois</button>
          </div>
        </div>
        <div class="usage-chart" id="usage-chart"></div>
      </section>

      <div class="usage-insights">
        <section class="usage-insight-card" aria-label="Itinéraires">
          <div class="usage-insight-head">
            <div class="usage-traffic-title">
              <h2>Itinéraires</h2>
              <span class="usage-traffic-total" id="usage-routes-summary"></span>
            </div>
            <div class="usage-segmented" id="usage-routes-periods" role="group" aria-label="Période des itinéraires">
              <button type="button" data-route-window="24h" class="active">24 h</button>
              <button type="button" data-route-window="7d">7 jours</button>
              <button type="button" data-route-window="all">Tout</button>
            </div>
          </div>
          <div class="usage-mini-stats" id="usage-routes-stats"></div>
          <div class="usage-table-wrap">
            <table class="usage-table">
              <thead>
                <tr>
                  <th>Date</th>
                  <th>Trajet</th>
                  <th>Distance</th>
                  <th>Type</th>
                  <th>Statut</th>
                  <th>Progression</th>
                </tr>
              </thead>
              <tbody id="usage-routes-body"></tbody>
            </table>
            <div class="usage-empty-list" id="usage-routes-empty" hidden>
              Aucun itinéraire sur cette période.
            </div>
          </div>
        </section>

        <section class="usage-insight-card" aria-label="Contributions">
          <div class="usage-insight-head">
            <div class="usage-traffic-title">
              <h2>Contributions</h2>
              <span class="usage-traffic-total" id="usage-contrib-summary"></span>
            </div>
            <div class="usage-segmented" id="usage-contrib-periods" role="group" aria-label="Période des contributions">
              <button type="button" data-contrib-window="24h">24 h</button>
              <button type="button" data-contrib-window="7d">7 jours</button>
              <button type="button" data-contrib-window="all" class="active">Tout</button>
            </div>
          </div>
          <div class="usage-mini-stats" id="usage-contrib-stats"></div>
          <div class="usage-contributors" id="usage-contributors"></div>
          <div class="usage-contrib-list" id="usage-contrib-list"></div>
        </section>
      </div>

      <div class="usage-layout">
        <section class="usage-map-card" aria-label="Carte des sessions">
          <div class="usage-map-toolbar">
            <div class="usage-segmented" role="group" aria-label="Période affichée">
              <button type="button" data-window="live" class="active">En direct</button>
              <button type="button" data-window="24h">24 h</button>
              <button type="button" data-window="7d">7 jours</button>
              <button type="button" data-window="all">Tout</button>
            </div>
            <span class="usage-map-note" id="usage-map-note">Chargement…</span>
          </div>
          <div class="usage-map-wrap">
            <div id="usage-map"></div>
            <div class="usage-empty" id="usage-empty">
              Aucune zone à afficher pour cette période.
            </div>
            <div class="usage-legend" aria-hidden="true">
              <span><i style="background: rgb(22 163 74)"></i> En direct</span>
              <span><i style="background: rgb(37 99 235)"></i> Moins de 24 h</span>
              <span><i style="background: rgb(148 163 184)"></i> Plus ancien</span>
            </div>
          </div>
        </section>

        <aside class="usage-sessions-card" aria-label="Sessions">
          <div class="usage-sessions-head">
            <h2>Sessions</h2>
            <span class="usage-sessions-count" id="usage-sessions-count"></span>
          </div>
          <div class="usage-detail" id="usage-detail" hidden></div>
          <div class="usage-sessions-list" id="usage-sessions-list"></div>
        </aside>
      </div>

      <div class="usage-route-preview" id="usage-route-preview" hidden aria-hidden="true">
        <div class="usage-route-preview-head">
          <strong id="usage-route-preview-title">Aperçu de l’itinéraire</strong>
          <span class="usage-route-preview-meta" id="usage-route-preview-meta"></span>
        </div>
        <div id="usage-route-preview-map"></div>
        <div class="usage-route-preview-note usage-muted" id="usage-route-preview-note"></div>
      </div>
    `;

    this.querySelectorAll('[data-window]').forEach((button) => {
      button.classList.toggle('active', button.dataset.window === this.filter);
      button.addEventListener('click', () => {
        if (this.filter === button.dataset.window) return;
        this.filter = button.dataset.window;
        this.querySelectorAll('[data-window]').forEach((candidate) => {
          candidate.classList.toggle('active', candidate === button);
        });
        this.updateUrl();
        this.clearSelection();
        this.hasFitBounds = false;
        this.refresh();
      });
    });

    this.querySelectorAll('[data-period]').forEach((button) => {
      button.classList.toggle('active', button.dataset.period === this.trafficPeriod);
      button.addEventListener('click', () => {
        if (this.trafficPeriod === button.dataset.period) return;
        this.trafficPeriod = button.dataset.period;
        this.querySelectorAll('[data-period]').forEach((candidate) => {
          candidate.classList.toggle('active', candidate === button);
        });
        this.updateTrafficUrl();
        this.refreshTraffic();
      });
    });

    this.querySelectorAll('[data-route-window]').forEach((button) => {
      button.classList.toggle('active', button.dataset.routeWindow === this.routeWindow);
      button.addEventListener('click', () => {
        if (this.routeWindow === button.dataset.routeWindow) return;
        this.routeWindow = button.dataset.routeWindow;
        this.querySelectorAll('[data-route-window]').forEach((candidate) => {
          candidate.classList.toggle('active', candidate === button);
        });
        this.refreshRoutes();
      });
    });

    this.querySelectorAll('[data-contrib-window]').forEach((button) => {
      button.classList.toggle('active', button.dataset.contribWindow === this.contributionWindow);
      button.addEventListener('click', () => {
        if (this.contributionWindow === button.dataset.contribWindow) return;
        this.contributionWindow = button.dataset.contribWindow;
        this.querySelectorAll('[data-contrib-window]').forEach((candidate) => {
          candidate.classList.toggle('active', candidate === button);
        });
        this.refreshContributions();
      });
    });

    this.initRoutePreview();
  }

  updateUrl() {
    const url = new URL(window.location);
    if (this.filter === 'live') {
      url.searchParams.delete('window');
    } else {
      url.searchParams.set('window', this.filter);
    }
    window.history.replaceState({}, '', url);
  }

  updateTrafficUrl() {
    const url = new URL(window.location);
    if (this.trafficPeriod === 'day') {
      url.searchParams.delete('period');
    } else {
      url.searchParams.set('period', this.trafficPeriod);
    }
    window.history.replaceState({}, '', url);
  }

  initMap() {
    this.map = new maplibregl.Map({
      container: this.querySelector('#usage-map'),
      style: '/style.json',
      center: [-73.57, 45.6],
      zoom: 6,
      minZoom: 3,
      maxZoom: 17,
      attributionControl: false,
    });

    this.map.addControl(new maplibregl.NavigationControl({ showCompass: false }), 'top-right');
    this.map.addControl(new maplibregl.AttributionControl({ compact: true }));

    this.map.on('load', () => {
      this.addUsageLayers();
      this.updateMapData();
    });

    this.map.on('click', 'usage-points', (event) => {
      const id = event.features?.[0]?.properties?.id;
      const session = this.sessions.find((candidate) => candidate.id === id);
      if (session) this.selectSession(session);
    });

    this.map.on('mouseenter', 'usage-points', () => {
      this.map.getCanvas().style.cursor = 'pointer';
    });
    this.map.on('mouseleave', 'usage-points', () => {
      this.map.getCanvas().style.cursor = '';
    });
  }

  addUsageLayers() {
    this.map.addSource('usage-sessions', {
      type: 'geojson',
      data: { type: 'FeatureCollection', features: [] },
    });

    this.map.addLayer({
      id: 'usage-heat',
      type: 'heatmap',
      source: 'usage-sessions',
      maxzoom: 15,
      paint: {
        'heatmap-weight': [
          'interpolate', ['linear'], ['get', 'page_views'], 1, 0.45, 20, 1,
        ],
        'heatmap-intensity': [
          'interpolate', ['linear'], ['zoom'], 3, 0.55, 10, 0.9,
        ],
        'heatmap-radius': [
          'interpolate', ['linear'], ['zoom'], 3, 20, 8, 32, 14, 54,
        ],
        'heatmap-opacity': [
          'interpolate', ['linear'], ['zoom'], 9, 0.55, 15, 0.15,
        ],
        'heatmap-color': [
          'interpolate', ['linear'], ['heatmap-density'],
          0, 'rgba(37, 99, 235, 0)',
          0.25, 'rgba(37, 99, 235, 0.32)',
          0.5, 'rgba(22, 163, 74, 0.5)',
          0.75, 'rgba(245, 158, 11, 0.66)',
          1, 'rgba(220, 38, 38, 0.8)',
        ],
      },
    });

    this.map.addLayer({
      id: 'usage-live-halo',
      type: 'circle',
      source: 'usage-sessions',
      filter: ['==', ['get', 'live'], true],
      paint: {
        'circle-radius': 17,
        'circle-color': 'rgb(22, 163, 74)',
        'circle-opacity': 0.16,
      },
    });

    this.map.addLayer({
      id: 'usage-points',
      type: 'circle',
      source: 'usage-sessions',
      paint: {
        'circle-radius': [
          'interpolate', ['linear'], ['zoom'],
          3, 5,
          8, 8,
          14, 12,
        ],
        'circle-color': [
          'case',
          ['==', ['get', 'live'], true], 'rgb(22, 163, 74)',
          ['==', ['get', 'recent'], true], 'rgb(37, 99, 235)',
          'rgb(148, 163, 184)',
        ],
        'circle-opacity': 0.88,
        'circle-stroke-color': 'white',
        'circle-stroke-width': 1.5,
      },
    });
  }

  updateMapData() {
    const now = Date.now();
    const features = this.sessions
      .filter((session) => session.lat != null && session.lng != null)
      .map((session) => {
        const ageSeconds = (now - new Date(session.last_seen).getTime()) / 1000;
        return {
          type: 'Feature',
          geometry: {
            type: 'Point',
            coordinates: [session.lng, session.lat],
          },
          properties: {
            id: session.id,
            live: session.live,
            recent: ageSeconds <= RECENT_SECONDS,
            page_views: session.page_views || 0,
            device: session.device || '',
            duration_seconds: session.duration_seconds || 0,
          },
        };
      });

    this.querySelector('#usage-map-note').textContent = features.length === 0
      ? 'Aucune zone'
      : `${numberFormat.format(features.length)} zone${features.length > 1 ? 's' : ''}`;
    const empty = this.querySelector('#usage-empty');
    empty.hidden = features.length > 0;
    if (features.length === 0) {
      empty.innerHTML = this.filter === 'live'
        ? `Aucune session en direct.<br>
           <span class="usage-muted">Les sessions historiques sont visibles
           avec « 24 h », « 7 jours » ou « Tout ».</span>`
        : 'Aucune zone à afficher pour cette période.';
    }

    // La carte peut encore être en cours de chargement : les compteurs et la
    // liste sont déjà à jour, on repassera ici au chargement du style.
    if (!this.map.getSource('usage-sessions')) return;

    this.map.getSource('usage-sessions').setData({
      type: 'FeatureCollection',
      features,
    });

    if (!this.hasFitBounds && features.length > 0) {
      const bounds = new maplibregl.LngLatBounds();
      features.forEach((feature) => bounds.extend(feature.geometry.coordinates));
      this.map.fitBounds(bounds, { padding: 70, maxZoom: 10, duration: 0 });
      this.hasFitBounds = true;
    }
  }

  async refresh() {
    const refresh = this.querySelector('#usage-refresh');
    const requestId = ++this.requestId;
    this.lastFetchAt = Date.now();
    try {
      const response = await fetch(
        `/usage/sessions?window=${encodeURIComponent(this.filter)}`,
        { headers: { Accept: 'application/json' }, cache: 'no-store' },
      );
      if (response.status === 401 || response.status === 403) {
        window.location.assign('/usage');
        return;
      }
      if (!response.ok) throw new Error(`HTTP ${response.status}`);

      const data = await response.json();
      if (requestId !== this.requestId) return;
      this.sessions = Array.isArray(data.sessions) ? data.sessions : [];
      this.stats = data.stats || null;

      this.updateStats();
      this.updateMapData();
      this.renderSessionList();

      if (this.selected) {
        const current = this.sessions.find((session) => session.id === this.selected.id);
        if (current) {
          this.selected = current;
          this.renderSessionDetail(current);
        } else {
          this.clearSelection();
        }
      }

      const now = new Date();
      this.querySelector('#usage-refresh-text').textContent = `Mis à jour à ${timeFormat.format(now)}`;
      refresh.classList.remove('error');

      if (this.lastTrafficAt && Date.now() - this.lastTrafficAt >= 60000) {
        this.refreshTraffic();
      }
      if (this.lastRoutesAt && Date.now() - this.lastRoutesAt >= 60000) {
        this.refreshRoutes();
      }
      if (this.lastContributionsAt
          && Date.now() - this.lastContributionsAt >= 60000) {
        this.refreshContributions();
      }
    } catch (error) {
      if (requestId !== this.requestId) return;
      console.error('Usage: chargement des statistiques', error);
      this.querySelector('#usage-refresh-text').textContent = 'Connexion perdue';
      refresh.classList.add('error');
      this.querySelector('#usage-empty').hidden = false;
    }
  }

  async refreshTraffic() {
    const requestId = ++this.trafficRequestId;
    this.lastTrafficAt = Date.now();
    try {
      const response = await fetch(
        `/usage/traffic?period=${encodeURIComponent(this.trafficPeriod)}`,
        { headers: { Accept: 'application/json' }, cache: 'no-store' },
      );
      if (response.status === 401 || response.status === 403) {
        window.location.assign('/usage');
        return;
      }
      if (!response.ok) throw new Error(`HTTP ${response.status}`);

      const data = await response.json();
      if (requestId !== this.trafficRequestId) return;
      this.traffic = data;
      this.renderTraffic();
    } catch (error) {
      if (requestId !== this.trafficRequestId) return;
      console.error('Usage: chargement du trafic', error);
      this.querySelector('#usage-chart').innerHTML =
        '<div class="usage-chart-empty">Impossible de charger le graphique.</div>';
    }
  }

  renderTraffic() {
    const chart = this.querySelector('#usage-chart');
    if (!chart || !this.traffic) return;

    const buckets = Array.isArray(this.traffic.buckets) ? this.traffic.buckets : [];
    const totals = this.traffic.totals || { sessions: 0, page_views: 0 };
    this.querySelector('#usage-traffic-total').textContent = totals.sessions
      ? `Sessions : ${numberFormat.format(totals.sessions)} · Pages vues : ${numberFormat.format(totals.page_views)}`
      : '';

    if (buckets.length === 0 || totals.sessions === 0) {
      chart.innerHTML = '<div class="usage-chart-empty">Aucune donnée sur cette période.</div>';
      return;
    }

    const width = Math.max(chart.clientWidth || 800, 320);
    const height = width < 640 ? 200 : 240;
    const padding = {
      top: 14,
      right: 12,
      bottom: 32,
      left: width < 640 ? 34 : 44,
    };
    const plotWidth = Math.max(10, width - padding.left - padding.right);
    const plotHeight = height - padding.top - padding.bottom;
    const maxSessions = Math.max(1, ...buckets.map((bucket) => bucket.sessions));
    const yMax = niceMax(maxSessions);
    const step = plotWidth / buckets.length;
    const barWidth = Math.max(2, Math.min(step * 0.68, 44));
    const labelEvery = Math.max(
      1,
      Math.ceil(buckets.length / (width < 640 ? 5 : 8)),
    );
    const parts = [];

    for (let index = 0; index <= 4; index += 1) {
      const value = (yMax * index) / 4;
      const y = padding.top + plotHeight - (value / yMax) * plotHeight;
      parts.push(
        `<line class="usage-chart-grid" x1="${padding.left}" y1="${y.toFixed(1)}" x2="${(width - padding.right).toFixed(1)}" y2="${y.toFixed(1)}" />`,
      );
      parts.push(
        `<text class="usage-chart-axis" x="${padding.left - 6}" y="${(y + 4).toFixed(1)}" text-anchor="end">${numberFormat.format(Math.round(value))}</text>`,
      );
    }

    parts.push(
      `<line class="usage-chart-grid" x1="${padding.left}" y1="${(padding.top + plotHeight).toFixed(1)}" x2="${(width - padding.right).toFixed(1)}" y2="${(padding.top + plotHeight).toFixed(1)}" />`,
    );

    buckets.forEach((bucket, index) => {
      const x = padding.left + index * step + (step - barWidth) / 2;
      const barHeight = bucket.sessions > 0
        ? Math.max(1, (bucket.sessions / yMax) * plotHeight)
        : 0;
      const y = padding.top + plotHeight - barHeight;
      const full = formatBucketFull(bucket.bucket, this.trafficPeriod);
      const label = formatBucketLabel(bucket.bucket, this.trafficPeriod);

      parts.push(
        `<rect class="usage-chart-bar" x="${x.toFixed(1)}" y="${y.toFixed(1)}" width="${barWidth.toFixed(1)}" height="${barHeight.toFixed(1)}" rx="2"><title>${escapeHtml(full)} : ${numberFormat.format(bucket.sessions)} sessions, ${numberFormat.format(bucket.page_views)} pages vues</title></rect>`,
      );

      if (index % labelEvery === 0) {
        const labelX = padding.left + index * step + step / 2;
        parts.push(
          `<text class="usage-chart-axis" x="${labelX.toFixed(1)}" y="${height - 10}" text-anchor="middle">${escapeHtml(label)}</text>`,
        );
      }
    });

    chart.innerHTML = `<svg viewBox="0 0 ${width} ${height}" role="img" aria-label="${escapeHtml(`Trafic : ${numberFormat.format(totals.sessions)} sessions`)}">${parts.join('')}</svg>`;
  }

  async refreshRoutes() {
    const requestId = ++this.routesRequestId;
    this.lastRoutesAt = Date.now();
    try {
      const response = await fetch(
        `/usage/routes?window=${encodeURIComponent(this.routeWindow)}`,
        { headers: { Accept: 'application/json' }, cache: 'no-store' },
      );
      if (response.status === 401 || response.status === 403) {
        window.location.assign('/usage');
        return;
      }
      if (!response.ok) throw new Error(`HTTP ${response.status}`);

      const data = await response.json();
      if (requestId !== this.routesRequestId) return;
      this.routes = data;
      this.renderRoutes();
    } catch (error) {
      if (requestId !== this.routesRequestId) return;
      console.error('Usage: chargement des itinéraires', error);
      this.querySelector('#usage-routes-body').innerHTML = '';
      const empty = this.querySelector('#usage-routes-empty');
      empty.hidden = false;
      empty.textContent = 'Impossible de charger les itinéraires.';
    }
  }

  initRoutePreview() {
    this.routePreview = this.querySelector('#usage-route-preview');
    const body = this.querySelector('#usage-routes-body');
    if (!this.routePreview || !body) return;

    const rowFromEvent = (event) => {
      const target = event.target instanceof Element ? event.target : null;
      return target?.closest('#usage-routes-body tr[data-route-id]') || null;
    };

    body.addEventListener('mouseover', (event) => {
      const row = rowFromEvent(event);
      if (row) this.scheduleRoutePreview(row);
    });

    body.addEventListener('mouseout', (event) => {
      const row = rowFromEvent(event);
      if (!row) return;
      if (event.relatedTarget instanceof Node && row.contains(event.relatedTarget)) return;
      this.cancelRoutePreview();
      this.scheduleRoutePreviewHide();
    });

    body.addEventListener('focusin', (event) => {
      const row = rowFromEvent(event);
      if (row) this.scheduleRoutePreview(row);
    });

    body.addEventListener('focusout', (event) => {
      const row = rowFromEvent(event);
      if (!row) return;
      if (event.relatedTarget instanceof Node && row.contains(event.relatedTarget)) return;
      this.cancelRoutePreview();
      this.scheduleRoutePreviewHide();
    });

    this.onPreviewScroll = () => this.hideRoutePreview();
    window.addEventListener('scroll', this.onPreviewScroll, true);
  }

  scheduleRoutePreview(row) {
    if (window.matchMedia && window.matchMedia('(hover: none)').matches) return;
    this.cancelRoutePreview();
    window.clearTimeout(this.routePreviewHideTimer);
    const routeId = row.dataset.routeId;
    if (this.routePreviewRouteId === routeId && !this.routePreview.hidden) return;
    this.routePreviewTimer = window.setTimeout(() => {
      this.showRoutePreview(row);
    }, 220);
  }

  cancelRoutePreview() {
    if (!this.routePreviewTimer) return;
    window.clearTimeout(this.routePreviewTimer);
    this.routePreviewTimer = null;
  }

  scheduleRoutePreviewHide() {
    window.clearTimeout(this.routePreviewHideTimer);
    this.routePreviewHideTimer = window.setTimeout(() => {
      this.hideRoutePreview();
    }, 120);
  }

  hideRoutePreview() {
    this.cancelRoutePreview();
    window.clearTimeout(this.routePreviewHideTimer);
    if (!this.routePreview) return;
    this.routePreview.hidden = true;
    this.routePreview.setAttribute('aria-hidden', 'true');
    this.routePreviewRouteId = null;
    this.routePreviewToken += 1;
    if (this.routePreviewAbort) {
      this.routePreviewAbort.abort();
      this.routePreviewAbort = null;
    }
  }

  showRoutePreview(row) {
    const id = row.dataset.routeId;
    const route = (this.routes?.routes || []).find((item) => item.id === id);
    if (!route || !this.routePreview) return;

    this.routePreviewRouteId = id;
    this.routePreview.hidden = false;
    this.routePreview.setAttribute('aria-hidden', 'false');

    const fast = route.chosen_route === 'fast';
    const distance = fast ? route.fast_distance_km : route.safe_distance_km;
    const title = `${fast ? 'Rapide' : 'Sécurisé'}${distance != null ? ` · ${formatKm(distance)}` : ''}`;
    this.querySelector('#usage-route-preview-title').textContent = title;
    this.querySelector('#usage-route-preview-meta').textContent = relativeTime(route.created_at);
    this.setRoutePreviewNote('Calcul du tracé…');
    this.positionRoutePreview(row);

    this.ensureRoutePreviewMap();
    this.loadRoutePreview(route);
  }

  positionRoutePreview(row) {
    const preview = this.routePreview;
    if (!preview) return;
    const rect = row.getBoundingClientRect();
    const gap = 12;
    const width = preview.offsetWidth || 340;
    const height = preview.offsetHeight || 240;
    const viewportWidth = window.innerWidth;
    const viewportHeight = window.innerHeight;
    let left = rect.right + gap;
    if (left + width > viewportWidth - 8) left = rect.left - width - gap;
    if (left < 8) {
      left = Math.min(Math.max(8, rect.left), Math.max(8, viewportWidth - width - 8));
    }
    let top = rect.top - 8;
    if (top + height > viewportHeight - 8) top = viewportHeight - height - 8;
    top = Math.max(8, top);
    preview.style.left = `${Math.round(left)}px`;
    preview.style.top = `${Math.round(top)}px`;
  }

  setRoutePreviewNote(text) {
    const note = this.querySelector('#usage-route-preview-note');
    if (note) note.textContent = text;
  }

  ensureRoutePreviewMap() {
    if (this.routePreviewMap) return;
    const container = this.querySelector('#usage-route-preview-map');
    if (!container) return;

    const map = new maplibregl.Map({
      container,
      style: '/style.json',
      center: [-73.57, 45.6],
      zoom: 8,
      interactive: false,
      attributionControl: false,
    });
    this.routePreviewMap = map;

    map.on('load', () => {
      if (this.routePreviewMap !== map) return;
      map.addSource('usage-preview-route', {
        type: 'geojson',
        data: { type: 'FeatureCollection', features: [] },
      });
      map.addLayer({
        id: 'usage-preview-route-casing',
        type: 'line',
        source: 'usage-preview-route',
        filter: ['==', ['geometry-type'], 'LineString'],
        layout: { 'line-cap': 'round', 'line-join': 'round' },
        paint: {
          'line-color': '#ffffff',
          'line-width': 9,
          'line-opacity': 0.9,
        },
      });
      map.addLayer({
        id: 'usage-preview-route-line',
        type: 'line',
        source: 'usage-preview-route',
        filter: ['==', ['geometry-type'], 'LineString'],
        layout: { 'line-cap': 'round', 'line-join': 'round' },
        paint: {
          'line-color': '#2563eb',
          'line-width': 5,
          'line-opacity': 0.95,
        },
      });
      map.addLayer({
        id: 'usage-preview-route-points',
        type: 'circle',
        source: 'usage-preview-route',
        filter: ['==', ['geometry-type'], 'Point'],
        paint: {
          'circle-radius': 6,
          'circle-color': [
            'match', ['get', 'role'],
            'start', '#dc2626',
            'end', '#2563eb',
            '#71717a',
          ],
          'circle-stroke-width': 2,
          'circle-stroke-color': '#ffffff',
        },
      });
      this.routePreviewReady = true;
      this.drawRoutePreview();
    });
  }

  routePreviewFeatures() {
    const coordinates = this.routePreviewCoords || [];
    const features = [];
    if (this.routePreviewFound && coordinates.length >= 2) {
      features.push({
        type: 'Feature',
        properties: {},
        geometry: { type: 'LineString', coordinates },
      });
    }
    if (coordinates.length >= 1) {
      features.push({
        type: 'Feature',
        properties: { role: 'start' },
        geometry: { type: 'Point', coordinates: coordinates[0] },
      });
      features.push({
        type: 'Feature',
        properties: { role: 'end' },
        geometry: { type: 'Point', coordinates: coordinates[coordinates.length - 1] },
      });
    }
    return { type: 'FeatureCollection', features };
  }

  drawRoutePreview() {
    const map = this.routePreviewMap;
    if (!map || !this.routePreviewReady) return;
    const source = map.getSource('usage-preview-route');
    if (!source) return;

    const coordinates = this.routePreviewCoords;
    if (!coordinates || coordinates.length === 0) {
      source.setData({ type: 'FeatureCollection', features: [] });
      return;
    }

    source.setData(this.routePreviewFeatures());
    const bounds = new maplibregl.LngLatBounds(coordinates[0], coordinates[0]);
    coordinates.forEach((coordinate) => bounds.extend(coordinate));
    map.fitBounds(bounds, { padding: 26, maxZoom: 14, duration: 0 });
  }

  async loadRoutePreview(route) {
    const token = ++this.routePreviewToken;
    const cacheKey = `${route.id}:${route.chosen_route === 'fast' ? 'fast' : 'safe'}`;
    const cached = this.routePreviewCache.get(cacheKey);
    if (cached) {
      this.routePreviewCoords = cached.coordinates;
      this.routePreviewFound = cached.found;
      this.setRoutePreviewNote(cached.found
        ? 'Tracé de l’itinéraire.'
        : 'Aucun tracé trouvé pour cet itinéraire.');
      this.drawRoutePreview();
      return;
    }

    // On affiche tout de suite départ et arrivée, puis le tracé dès que le
    // calcul est terminé.
    this.routePreviewCoords = [
      [route.start_lng, route.start_lat],
      [route.end_lng, route.end_lat],
    ];
    this.routePreviewFound = false;
    this.drawRoutePreview();
    this.setRoutePreviewNote('Calcul du tracé…');

    if (this.routePreviewAbort) this.routePreviewAbort.abort();
    const controller = new AbortController();
    this.routePreviewAbort = controller;

    try {
      const response = await fetch(
        `/usage/routes/${encodeURIComponent(route.id)}/geometry`,
        {
          headers: { Accept: 'application/json' },
          cache: 'no-store',
          signal: controller.signal,
        },
      );
      if (response.status === 401 || response.status === 403) {
        window.location.assign('/usage');
        return;
      }
      if (!response.ok) throw new Error(`HTTP ${response.status}`);

      const data = await response.json();
      if (token !== this.routePreviewToken) return;
      const coordinates = Array.isArray(data.coordinates)
        ? data.coordinates
          .filter((coordinate) => Array.isArray(coordinate) && coordinate.length >= 2)
          .map(([lng, lat]) => [Number(lng), Number(lat)])
          .filter(([lng, lat]) => Number.isFinite(lng) && Number.isFinite(lat))
        : [];
      const result = {
        coordinates,
        found: data.found === true && coordinates.length >= 3,
      };
      if (this.routePreviewCache.size >= ROUTE_PREVIEW_CACHE_LIMIT) {
        this.routePreviewCache.delete(this.routePreviewCache.keys().next().value);
      }
      this.routePreviewCache.set(cacheKey, result);
      this.routePreviewCoords = result.coordinates;
      this.routePreviewFound = result.found;
      this.setRoutePreviewNote(result.found
        ? 'Tracé de l’itinéraire.'
        : 'Aucun tracé trouvé pour cet itinéraire.');
      this.drawRoutePreview();
    } catch (error) {
      if (error.name === 'AbortError') return;
      if (token !== this.routePreviewToken) return;
      console.error('Usage: aperçu de l’itinéraire', error);
      this.setRoutePreviewNote('Impossible de charger l’aperçu.');
    } finally {
      if (this.routePreviewAbort === controller) this.routePreviewAbort = null;
    }
  }

  renderRoutes() {
    if (!this.routes) return;
    const stats = this.routes.stats || {};
    const routes = Array.isArray(this.routes.routes) ? this.routes.routes : [];

    this.querySelector('#usage-routes-summary').textContent = stats.requests
      ? `${numberFormat.format(stats.followed || 0)} suivis · ${numberFormat.format(stats.completed || 0)} terminés · ${Math.round((stats.completion_rate || 0) * 100)} %`
      : '';

    this.querySelector('#usage-routes-stats').innerHTML = [
      [stats.requests, 'demandes'],
      [stats.followed, 'suivis'],
      [stats.completed, 'terminés'],
      [stats.abandoned, 'abandonnés'],
    ].map(([value, label]) => (
      `<span class="usage-mini-stat"><strong>${numberFormat.format(value || 0)}</strong> ${label}</span>`
    )).join('');

    const body = this.querySelector('#usage-routes-body');
    const empty = this.querySelector('#usage-routes-empty');
    if (this.routePreview && !this.routePreview.hidden) this.hideRoutePreview();
    if (routes.length === 0) {
      body.innerHTML = '';
      empty.hidden = false;
      empty.textContent = 'Aucun itinéraire sur cette période.';
      return;
    }
    empty.hidden = true;

    body.innerHTML = routes.map((route) => {
      const start = `${route.start_lat.toFixed(3)}, ${route.start_lng.toFixed(3)}`;
      const end = `${route.end_lat.toFixed(3)}, ${route.end_lng.toFixed(3)}`;
      let chosenDistance = null;
      if (route.chosen_route === 'safe') chosenDistance = route.safe_distance_km;
      else if (route.chosen_route === 'fast') chosenDistance = route.fast_distance_km;
      const distanceLabel = chosenDistance != null
        ? formatKm(chosenDistance)
        : ([
            route.safe_distance_km != null ? formatKm(route.safe_distance_km) : null,
            route.fast_distance_km != null ? formatKm(route.fast_distance_km) : null,
          ].filter(Boolean).join(' / ') || '—');
      const progress = Math.round((route.progress || 0) * 100);
      const statusClass = ['completed', 'abandoned', 'following'].includes(route.status)
        ? ` ${route.status}`
        : '';
      const device = [route.device, route.browser, route.os].filter(Boolean).join(' · ');
      const exactDate = escapeHtml(new Date(route.created_at).toLocaleString('fr-CA'));
      return `
        <tr class="usage-route-row" data-route-id="${escapeHtml(route.id)}" tabindex="0">
          <td title="${exactDate}">${escapeHtml(relativeTime(route.created_at))}</td>
          <td>
            <span class="usage-route-trajet">${escapeHtml(start)} → ${escapeHtml(end)}</span>
            ${device ? `<div class="usage-muted">${escapeHtml(device)}</div>` : ''}
          </td>
          <td>${escapeHtml(distanceLabel)}</td>
          <td>${escapeHtml(routeTypeLabel(route.chosen_route))}</td>
          <td><span class="usage-badge${statusClass}">${escapeHtml(routeStatusLabel(route.status))}</span></td>
          <td>
            <div class="usage-progress">
              <span class="usage-progress-track"><span class="usage-progress-fill" style="width:${Math.min(100, Math.max(0, progress))}%"></span></span>
              <span>${progress} %</span>
            </div>
          </td>
        </tr>
      `;
    }).join('');
  }

  async refreshContributions() {
    const requestId = ++this.contributionsRequestId;
    this.lastContributionsAt = Date.now();
    try {
      const response = await fetch(
        `/usage/contributions?window=${encodeURIComponent(this.contributionWindow)}`,
        { headers: { Accept: 'application/json' }, cache: 'no-store' },
      );
      if (response.status === 401 || response.status === 403) {
        window.location.assign('/usage');
        return;
      }
      if (!response.ok) throw new Error(`HTTP ${response.status}`);

      const data = await response.json();
      if (requestId !== this.contributionsRequestId) return;
      this.contributions = data;
      this.renderContributions();
    } catch (error) {
      if (requestId !== this.contributionsRequestId) return;
      console.error('Usage: chargement des contributions', error);
      this.querySelector('#usage-contrib-list').innerHTML =
        '<div class="usage-empty-list">Impossible de charger les contributions.</div>';
    }
  }

  renderContributions() {
    if (!this.contributions) return;
    const stats = this.contributions.stats || {};
    const items = Array.isArray(this.contributions.contributions)
      ? this.contributions.contributions
      : [];

    this.querySelector('#usage-contrib-summary').textContent = stats.reports || stats.comments
      ? `${numberFormat.format(stats.reports || 0)} contributions · ${numberFormat.format(stats.comments || 0)} commentaires`
      : '';

    this.querySelector('#usage-contrib-stats').innerHTML = [
      [stats.reports, 'contributions'],
      [stats.comments, 'commentaires'],
      [stats.photos, 'photos'],
      [stats.contributors, 'contributeurs'],
    ].map(([value, label]) => (
      `<span class="usage-mini-stat"><strong>${numberFormat.format(value || 0)}</strong> ${label}</span>`
    )).join('');

    const contributors = Array.isArray(this.contributions.top_contributors)
      ? this.contributions.top_contributors
      : [];
    this.querySelector('#usage-contributors').innerHTML = contributors.map((contributor) => (
      `<span class="usage-contributor"><strong>${escapeHtml(contributor.name)}</strong> ${numberFormat.format(contributor.contributions)}</span>`
    )).join('');

    const list = this.querySelector('#usage-contrib-list');
    if (items.length === 0) {
      list.innerHTML = '<div class="usage-empty-list">Aucune contribution sur cette période.</div>';
      return;
    }

    list.innerHTML = items.map((item) => {
      const thumb = safeImageUrl(item.photo_path_thumbnail);
      const score = scoreLabel(item.score);
      const detail = item.kind === 'comment'
        ? [item.comment || '', score !== '—' ? `Score ${score}` : ''].filter(Boolean).join(' · ')
        : [item.label || 'Signalement', score !== '—' ? `Score ${score}` : ''].filter(Boolean).join(' · ');
      const link = `/?report=${item.report_id}`;
      const exactDate = escapeHtml(new Date(item.created_at).toLocaleString('fr-CA'));
      return `
        <div class="usage-contrib-item">
          ${thumb ? `<img class="usage-contrib-thumb" src="${escapeHtml(thumb)}" alt="" loading="lazy">` : ''}
          <div class="usage-contrib-body">
            <div class="usage-contrib-head">
              <span class="usage-badge">${item.kind === 'comment' ? 'Commentaire' : 'Contribution'}</span>
              <strong>${escapeHtml(item.user_name)}</strong>
              <span class="usage-muted" title="${exactDate}">${escapeHtml(relativeTime(item.created_at))}</span>
            </div>
            <div class="usage-contrib-text">${escapeHtml(detail)}</div>
            <a class="usage-contrib-link" href="${escapeHtml(link)}">Voir sur la carte →</a>
          </div>
        </div>
      `;
    }).join('');
  }

  updateStats() {
    if (!this.stats) return;
    this.querySelector('#usage-stat-live').textContent = numberFormat.format(this.stats.live);
    this.querySelector('#usage-stat-24h').textContent = numberFormat.format(this.stats.sessions_24h);
    this.querySelector('#usage-stat-pages').textContent = numberFormat.format(this.stats.page_views_24h);
    this.querySelector('#usage-stat-actions').textContent = numberFormat.format(this.stats.actions_24h);
    this.querySelector('#usage-stat-duration').textContent = formatDuration(this.stats.avg_duration_seconds);
    this.querySelector('#usage-stat-total').textContent = numberFormat.format(this.stats.sessions_total);
  }

  renderSessionList() {
    const list = this.querySelector('#usage-sessions-list');
    const count = this.querySelector('#usage-sessions-count');
    const total = this.sessions.length;
    const shown = Math.min(total, LIST_LIMIT);
    count.textContent = total
      ? (total > shown
        ? `${numberFormat.format(shown)} sur ${numberFormat.format(total)} sessions`
        : `${numberFormat.format(total)} session${total > 1 ? 's' : ''}`)
      : '';

    if (total === 0) {
      list.innerHTML = '<div class="usage-empty-list">Aucune session sur cette période.</div>';
      return;
    }

    const now = Date.now();
    const fragment = document.createDocumentFragment();

    this.sessions.slice(0, shown).forEach((session) => {
      const ageSeconds = (now - new Date(session.last_seen).getTime()) / 1000;
      const recent = ageSeconds <= RECENT_SECONDS;
      const item = document.createElement('button');
      item.type = 'button';
      item.className = [
        'usage-session',
        session.live ? 'live' : '',
        !session.live && recent ? 'recent' : '',
        this.selected?.id === session.id ? 'selected' : '',
      ].filter(Boolean).join(' ');
      item.dataset.id = session.id;
      item.innerHTML = html`
        <span class="usage-session-dot" aria-hidden="true"></span>
        <span class="usage-session-body">
          <span class="usage-session-top">
            <span class="usage-session-client">${escapeHtml(describeClient(session))}</span>
            <span class="usage-session-ago" title="${escapeHtml(dateTimeFormat.format(new Date(session.last_seen)))}">${escapeHtml(relativeTime(session.last_seen))}</span>
          </span>
          <span class="usage-session-meta">
            ${escapeHtml(formatDuration(session.duration_seconds))} ·
            ${numberFormat.format(session.page_views)} page(s) ·
            ${escapeHtml(sessionPlace(session))}
          </span>
        </span>
      `;
      item.addEventListener('click', () => this.selectSession(session));
      fragment.appendChild(item);
    });

    list.replaceChildren(fragment);
  }

  renderSessionDetail(session) {
    const detail = this.querySelector('#usage-detail');
    const referrer = referrerText(session);
    const lastSeenExact = escapeHtml(
      dateTimeFormat.format(new Date(session.last_seen)),
    );
    detail.hidden = false;
    detail.innerHTML = html`
      <div class="usage-detail-head">
        <strong>Session ${escapeHtml(session.id.slice(0, 8))}</strong>
        <span class="usage-badge ${session.live ? 'live' : ''}">
          ${session.live ? 'En direct' : 'Terminée'}
        </span>
        <button type="button" class="usage-detail-close" aria-label="Fermer le détail">✕</button>
      </div>
      <dl>
        <dt>Commencée</dt>
        <dd>${escapeHtml(dateTimeFormat.format(new Date(session.started_at)))}</dd>
        <dt>Durée</dt>
        <dd>${escapeHtml(formatDuration(session.duration_seconds))}</dd>
        <dt>Dernière activité</dt>
        <dd title="${lastSeenExact}">${escapeHtml(relativeTime(session.last_seen))}</dd>
        <dt>Pages vues</dt>
        <dd>${numberFormat.format(session.page_views)}</dd>
        <dt>Appareil</dt>
        <dd>${escapeHtml(describeClient(session))}</dd>
        <dt>Langue</dt>
        <dd>${escapeHtml(session.language || 'inconnue')}</dd>
        <dt>Lieu</dt>
        <dd>
          ${session.default_view
            ? `Vue par défaut <span class="usage-muted">· ${escapeHtml(positionText(session))}</span>`
            : session.city
              ? `${escapeHtml(session.city)} <span class="usage-muted">· ${escapeHtml(positionText(session))}</span>`
              : escapeHtml(positionText(session))}
        </dd>
        <dt>Page</dt>
        <dd>${escapeHtml(session.path || 'inconnue')}</dd>
        ${referrer ? `<dt>Référent</dt><dd>${escapeHtml(referrer)}</dd>` : ''}
        ${sourceLabel(session)
          ? `<dt>Source</dt><dd>${escapeHtml(sourceLabel(session))}</dd>`
          : ''}
      </dl>
    `;
    detail.querySelector('.usage-detail-close').addEventListener('click', () => {
      this.clearSelection();
    });
  }

  selectSession(session) {
    this.removePopup();
    this.selected = session;
    this.renderSessionList();
    this.renderSessionDetail(session);

    if (session.lat == null || session.lng == null) return;

    this.map.flyTo({
      center: [session.lng, session.lat],
      zoom: Math.max(this.map.getZoom(), 12),
      duration: 700,
    });

    this.popup = new maplibregl.Popup({ offset: 14, maxWidth: '320px' })
      .setLngLat([session.lng, session.lat])
      .setHTML(buildPopupHtml(session))
      .addTo(this.map);

    const selectedId = session.id;
    this.popup.on('close', () => {
      if (this.ignorePopupClose) return;
      if (this.selected?.id === selectedId) {
        this.selected = null;
        this.popup = null;
        this.querySelector('#usage-detail').hidden = true;
        this.renderSessionList();
      }
    });
  }

  removePopup() {
    if (!this.popup) return;
    this.ignorePopupClose = true;
    const popup = this.popup;
    this.popup = null;
    popup.remove();
    this.ignorePopupClose = false;
  }

  clearSelection() {
    this.selected = null;
    this.querySelector('#usage-detail').hidden = true;
    this.renderSessionList();
    this.removePopup();
  }
}

customElements.define('vi-usage', ViUsage);
