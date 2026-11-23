// Render a compact, dependency-free README growth chart from Repo Growth snapshots.
// Uses observed data only; never interpolates missing daily samples.
import fs from 'node:fs';
import path from 'node:path';

const input = '.github/repo-growth/history.json';
const output = 'assets/repo-growth.svg';
const h = JSON.parse(fs.readFileSync(input, 'utf8'));
// Mirror landing/functions/api/beta-downloads.ts: only executable installers
// and portable ZIPs are application downloads. Never count manifests/checksums.
const INSTALLER_NAME = /^otto-os-setup-\d+(?:\.\d+)*-x64\.exe$/i;
const PORTABLE_ZIP_NAME = /^otto-os-\d+(?:\.\d+)*(?:-[0-9A-Za-z.-]+)?-windows-x64\.zip$/i;
const isApplicationAsset = (name) =>
  INSTALLER_NAME.test(name) || PORTABLE_ZIP_NAME.test(name);
const applicationAssets = Object.values(h.assets ?? {})
  .filter((asset) => asset?.active !== false && isApplicationAsset(asset?.name ?? ''));
const countDownloads = (asset) => Math.max(0, Number(asset?.lastSeen ?? asset?.maximumSeen) || 0);
const total = applicationAssets.reduce((sum, asset) => sum + countDownloads(asset), 0);

// Record a separate daily series because Repo Growth's built-in snapshots
// include checksums and manifests. Never relabel older raw snapshots.
const appHistoryPath = '.github/repo-growth/application-history.json';
const appHistory = fs.existsSync(appHistoryPath)
  ? JSON.parse(fs.readFileSync(appHistoryPath, 'utf8'))
  : { version: 1, repository: h.repository, points: [] };
const snapshotDate = (h.updatedAt ?? '').slice(0, 10);
if (!/^\d{4}-\d{2}-\d{2}$/.test(snapshotDate)) {
  throw new Error('Repo Growth history is missing a valid updatedAt date');
}
const originalPoint = (h.points ?? []).find((p) => p.date === snapshotDate) ?? (h.points ?? []).at(-1) ?? {};
const snapshot = {
  date: snapshotDate,
  downloads: total,
  stars: Number(originalPoint.stars) || 0,
  forks: Number(originalPoint.forks) || 0
};
const points = (appHistory.points ?? [])
  .filter((p) => p && /^\d{4}-\d{2}-\d{2}$/.test(p.date) && Number.isFinite(p.downloads))
  .filter((p) => p.date !== snapshotDate)
  .concat(snapshot)
  .sort((a, b) => a.date.localeCompare(b.date));
fs.writeFileSync(appHistoryPath, JSON.stringify({
  version: 1, repository: h.repository, points
}, null, 2) + '\n');
const last = points.at(-1) ?? {};
const stars = snapshot.stars;
const forks = snapshot.forks;
const number = (n) => Math.max(0, Number(n) || 0).toLocaleString('en-US');
const esc = (s) => String(s).replace(/[&<>"']/g, (c) => ({
  '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&apos;'
})[c]);
const date = (s) => s
  ? new Date(s + 'T00:00:00Z').toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric', timeZone: 'UTC' })
  : 'Not collected';
const releaseTotals = new Map();
for (const asset of applicationAssets) {
  if (!asset?.release) continue;
  releaseTotals.set(asset.release, (releaseTotals.get(asset.release) || 0) + countDownloads(asset));
}
const releases = [...releaseTotals].sort(([a], [b]) => {
  const parts = (v) => v.replace(/^v/, '').split('.').map((n) => Number(n) || 0);
  const x = parts(a), y = parts(b);
  return (x[0] - y[0]) || (x[1] - y[1]) || (x[2] - y[2]);
}).slice(-5);
const S = [];
const add = (v) => S.push(v);
const txt = (x, y, value, css, more = '') =>
  add('<text x="' + x + '" y="' + y + '" class="' + css + '" ' + more + '>' + esc(value) + '</text>');
const line = (x1, y1, x2, y2, css) =>
  add('<line x1="' + x1 + '" y1="' + y1 + '" x2="' + x2 + '" y2="' + y2 + '" class="' + css + '"/>');
const rect = (x, y, w, height, radius, css) =>
  add('<rect x="' + x + '" y="' + y + '" width="' + w + '" height="' + height + '" rx="' + radius + '" class="' + css + '"/>');
add('<?xml version="1.0" encoding="UTF-8"?>');
add('<svg xmlns="http://www.w3.org/2000/svg" width="1000" height="510" viewBox="0 0 1000 510" role="img" aria-labelledby="title desc">');
add('<title id="title">Otto OS adoption and GitHub release statistics</title>');
add('<desc id="desc">Observed application downloads: ' + number(total) + ', stars: ' + number(stars) + ', forks: ' + number(forks) + '. ' + points.length + ' daily snapshots. The bar chart shows download counts by release.</desc>');
add('<style><![CDATA[');
add('text{font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Arial,sans-serif}');
add('.bg{fill:#fff}.border{fill:none;stroke:#e5e7eb;stroke-width:1}.divider,.grid{stroke:#e5e7eb;stroke-width:1}');
add('.heading{fill:#171b24;font-size:25px;font-weight:700;letter-spacing:-.5px}');
add('.eyebrow{fill:#757b88;font-size:12px;font-weight:650;letter-spacing:1.4px}');
add('.meta{fill:#757b88;font-size:13px}.value{fill:#151922;font-size:37px;font-weight:720;letter-spacing:-1.2px}');
add('.panel{fill:#1f2937;font-size:17px;font-weight:650}.tick{fill:#858b98;font-size:12px}');
add('.metric{fill:#1e61f0}.barbg{fill:#e9eef7}.bar{fill:#3b76ed}.trend{fill:none;stroke:#2965ed;stroke-width:3;stroke-linejoin:round;stroke-linecap:round}');
add('.pt{fill:#2965ed;stroke:#fff;stroke-width:2}.mark{stroke:#a9b9de;stroke-dasharray:4 5;stroke-width:1.5}');
add('@media(prefers-color-scheme:dark){.bg{fill:#0d1117}.border,.divider,.grid{stroke:#30363d}.heading,.panel,.value{fill:#e6edf3}.eyebrow,.meta,.tick{fill:#8b949e}.barbg{fill:#222c3d}.bar{fill:#4884ff}.trend{stroke:#72a1ff}.pt{fill:#72a1ff;stroke:#0d1117}.mark{stroke:#48597b}.metric{fill:#72a1ff}}');
add(']]></style>');
rect(1, 1, 998, 508, 18, 'bg');
rect(1, 1, 998, 508, 18, 'border');
rect(32, 32, 8, 24, 4, 'metric');
txt(54, 54, 'OTTO OS', 'heading');
txt(54, 73, 'OPEN-SOURCE DESKTOP SHELL / WINDOWS 11', 'eyebrow');
txt(966, 53, 'GITHUB RELEASE METRICS', 'eyebrow', 'text-anchor="end"');
txt(966, 72, 'Updated ' + date(last.date), 'meta', 'text-anchor="end"');
line(32, 92, 968, 92, 'divider');

const metrics = [
  { x: 36, label: 'APPLICATION DOWNLOADS', value: total },
  { x: 389, label: 'GITHUB STARS', value: stars },
  { x: 703, label: 'FORKS', value: forks }
];
for (const m of metrics) {
  txt(m.x, 125, m.label, 'eyebrow');
  txt(m.x, 175, number(m.value), 'value');
}
line(32, 198, 968, 198, 'divider');
txt(36, 232, 'Downloads over time', 'panel');
txt(36, 251, 'Cumulative, daily snapshots', 'meta');
txt(700, 232, 'Downloads by release', 'panel');
txt(700, 251, 'Installer + portable ZIP only', 'meta');
line(671, 217, 671, 442, 'divider');

// Timeline: show a single observed point rather than fabricate a trend.
const left = 108, right = 630, top = 278, bottom = 422;
const max = Math.max(1, ...points.map((p) => Number(p.downloads) || 0));
const axisTop = Math.max(4, Math.ceil(max / 4) * 4);
for (let i = 0; i <= 4; i++) {
  const y = bottom - i * (bottom - top) / 4;
  line(left, y, right, y, 'grid');
  txt(left - 12, y + 4, number(axisTop * i / 4), 'tick', 'text-anchor="end"');
}
if (points.length >= 2) {
  const coords = points.map((p, i) => ({
    x: left + (i / (points.length - 1)) * (right - left),
    y: bottom - Math.min(axisTop, Math.max(0, Number(p.downloads) || 0)) / axisTop * (bottom - top)
  }));
  add('<path d="' + coords.map((p, i) => (i ? 'L' : 'M') + p.x.toFixed(2) + ' ' + p.y.toFixed(2)).join(' ') + '" class="trend"/>');
  const p = coords.at(-1);
  add('<circle cx="' + p.x.toFixed(2) + '" cy="' + p.y.toFixed(2) + '" r="5" class="pt"/>');
  txt(left, 449, date(points[0].date), 'tick');
  txt(right, 449, date(last.date), 'tick', 'text-anchor="end"');
} else if (points.length === 1) {
  const x = (left + right) / 2;
  const y = bottom - total / axisTop * (bottom - top);
  line(x, top, x, bottom, 'mark');
  add('<circle cx="' + x + '" cy="' + y.toFixed(2) + '" r="6" class="pt"/>');
  txt(x, top - 8, 'FIRST SNAPSHOT', 'eyebrow', 'text-anchor="middle"');
  txt(x, 450, date(last.date) + ' · Trend available after more snapshots', 'tick', 'text-anchor="middle"');
} else {
  txt((left + right) / 2, 360, 'No observations recorded yet', 'meta', 'text-anchor="middle"');
}
if (releases.length) {
  const topBar = Math.max(1, ...releases.map(([, value]) => value));
  releases.forEach(([name, count], i) => {
    const y = 286 + i * 36;
    txt(700, y + 2, name, 'meta');
    rect(782, y - 9, 148, 10, 5, 'barbg');
    rect(782, y - 9, 148 * count / topBar, 10, 5, 'bar');
    txt(965, y + 1, number(count), 'tick', 'text-anchor="end"');
  });
} else {
  txt(700, 345, 'No published releases yet', 'meta');
}
line(32, 465, 968, 465, 'divider');
txt(36, 489, 'SOURCE  ·  GITHUB RELEASE EXE + ZIP', 'eyebrow');
txt(966, 489, 'Installer and portable ZIP requests only; not unique users or installations.', 'tick', 'text-anchor="end"');
add('</svg>');
fs.mkdirSync(path.dirname(output), { recursive: true });
fs.writeFileSync(output, S.join('\n') + '\n');
console.log('Generated ' + output + ' from ' + points.length + ' snapshots and ' + releases.length + ' releases.');
