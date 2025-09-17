import '@material/web/all.js';
import maplibregl from 'maplibre-gl';
import './web-components/VeloinfoMap.js';
import './web-components/FollowPanel.js';
import './web-components/RoutePanel.js';
import './web-components/RouteSearching.js';
import './web-components/SearchInput.js';
import './web-components/VeloinfoMenu.js';
import './web-components/VeloinfoInstallIos.js';
import './web-components/VeloinfoInstallAndroid.js';
import './web-components/SnowPanel.js';
import './web-components/MobilizonEvents.js';
import './web-components/RouteDefine.js';
import htmx from 'htmx.org';

if ("serviceWorker" in navigator) {
    navigator.serviceWorker.register("/pub/service-worker.js");
}



const ex = { maplibregl };
Object.assign(window, ex);

export { htmx, maplibregl };